import { useState } from "react";
import type { ChangeDto, MarkColor, ReviewDto, View } from "../bindings";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { MarkBar } from "./MarkBar";
import { type MarkSetup, markVar } from "./marks";
import type { ViewMode } from "./model";
import type { Filter } from "./rowState";
import { type Progress, type ProgressPart, progressParts } from "./state";
import "./ReviewBar.css";

const SINCE_LABEL = {
  unknown: "",
  new: "new",
  changed: "changed",
  same: "unchanged",
  removed: "removed",
} as const;

const shortId = (c: ChangeDto) => c.changeId || c.commitId.slice(0, 12);

function changeLabel(c: ChangeDto) {
  const since = SINCE_LABEL[c.since];
  return `${shortId(c)} ${c.description || "(no description)"}${since ? ` · ${since}` : ""}`;
}

/** All, Not done, and each color in use, plus the current one if it was since removed. */
function filterOptions(setup: MarkSetup, current: Filter): [Filter, string][] {
  const colors = setup.shown.map((s) => s.color);
  if (current !== "all" && current !== "left" && !colors.includes(current)) colors.push(current);
  const label = (c: MarkColor) => setup.label(c).replace(/^./, (x) => x.toUpperCase());
  return [
    ["all", "All lines"],
    ["left", "Not done"],
    ...colors.map((c) => [c, label(c)] as [Filter, string]),
  ];
}

/** The change a per-change view starts on: the first one that changed since last session. */
function defaultChange(review: ReviewDto, kind: "change" | "sinceLast"): string | undefined {
  const pick =
    kind === "sinceLast"
      ? (review.changes.find((c) => c.since === "changed" || c.since === "new") ??
        review.changes[0])
      : review.changes[review.changes.length - 1];
  return pick?.changeId;
}

interface Props {
  review?: ReviewDto;
  onView(view: View): void;
  mode: ViewMode;
  onMode(mode: ViewMode): void;
  filter: Filter;
  onFilter(filter: Filter): void;
  progress: Progress;
  setup: MarkSetup;
  /** Marks the selection with a color. */
  onMark(color: MarkColor): void;
  onReset(): void;
  /** Hands focus back to the review once the reset confirm closes. */
  onDone(): void;
}

export function ReviewBar(props: Props) {
  const { review } = props;
  const view = review?.view ?? { kind: "combined" };
  const current = view.kind === "combined" ? undefined : view.change;

  function pickKind(kind: View["kind"]) {
    if (!review) return;
    if (kind === "combined") return props.onView({ kind });
    const change = current ?? defaultChange(review, kind);
    if (change) props.onView({ kind, change });
  }

  return (
    <div className="review-bar">
      <span className="quiet-select">
        <select
          value={view.kind}
          disabled={!review}
          onChange={(e) => pickKind(e.target.value as View["kind"])}
          aria-label="View"
        >
          <option value="combined">Combined</option>
          <option value="change">Per change</option>
          <option value="sinceLast" disabled={!review?.previousSession}>
            Since last session
          </option>
        </select>
      </span>
      {review && view.kind !== "combined" && (
        <span className="quiet-select change-picker">
          <select
            value={current}
            onChange={(e) => props.onView({ kind: view.kind, change: e.target.value })}
            aria-label="Change"
          >
            {review.changes.map((c) => (
              <option key={c.changeId} value={c.changeId}>
                {changeLabel(c)}
              </option>
            ))}
          </select>
        </span>
      )}
      <span className="quiet-select">
        <select
          value={props.filter}
          onChange={(e) => props.onFilter(e.target.value as Filter)}
          aria-label="Line filter"
        >
          {filterOptions(props.setup, props.filter).map(([f, label]) => (
            <option key={f} value={f}>
              {label}
            </option>
          ))}
        </select>
      </span>
      <span className="mode-toggle">
        <IconButton
          icon="unified"
          label="Unified"
          aria-pressed={props.mode === "unified"}
          onClick={() => props.onMode("unified")}
        />
        <IconButton
          icon="split"
          label="Split"
          aria-pressed={props.mode === "split"}
          onClick={() => props.onMode("split")}
        />
      </span>
      <span className="spacer" />
      <MarkBar
        setup={props.setup}
        onMark={review ? props.onMark : undefined}
        onFilter={props.onFilter}
      />
      {review && <ProgressBar progress={props.progress} setup={props.setup} />}
      <ResetMenu disabled={!review} onReset={props.onReset} onDone={props.onDone} />
    </div>
  );
}

const share = (n: number, of: number) => (of > 0 ? (n / of) * 100 : 0);

/** Marked rows as one bar split by color, then rows done by their file, with the done share
 * beside it. Hovering shows the lines reviewed and left, and each part's count. */
function ProgressBar({ progress: p, setup }: { progress: Progress; setup: MarkSetup }) {
  const [hover, setHover] = useState(false);
  const parts = progressParts(p, setup);
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: hovering only shows counts
    <span
      className="progress"
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      <span className="progress-stack">
        {parts
          .filter((x) => x.key !== "unmarked")
          .map((x) => (
            <span
              key={x.key}
              className={x.color ? undefined : "p-by-file"}
              style={{
                width: `${share(x.rows, p.changed)}%`,
                background: x.color && markVar(x.color),
              }}
            />
          ))}
      </span>
      <span className="progress-pct">{Math.floor(share(p.done, p.changed))}%</span>
      {hover && <ProgressCard p={p} parts={parts} />}
    </span>
  );
}

const count = (n: number) => n.toLocaleString("en-US");

function ProgressCard({ p, parts }: { p: Progress; parts: ProgressPart[] }) {
  const bar = [...parts.filter((x) => x.done), ...parts.filter((x) => !x.done)];
  return (
    <span className="progress-card">
      <span className="pc-totals">
        <span>
          <span className="pc-label">REVIEWED</span>
          <b>{count(p.done)}</b>
        </span>
        <span>
          <span className="pc-label">LEFT</span>
          <b className="pc-left">{count(p.changed - p.done)}</b>
        </span>
      </span>
      <span className="pc-bar">
        {bar
          .filter((x) => x.key !== "unmarked")
          .map((x) => (
            <span
              key={x.key}
              className={x.color ? undefined : "p-by-file"}
              style={{
                width: `${share(x.rows, p.changed)}%`,
                background: x.color && markVar(x.color),
              }}
            />
          ))}
      </span>
      <span className="pc-sep" />
      <span className="pc-rows">
        {parts.map((x) => (
          <span key={x.key} className="pc-row">
            <span
              className={`pc-sw${x.key === "byFile" ? " p-by-file" : ""}${x.key === "unmarked" ? " empty" : ""}`}
              style={x.color ? { background: markVar(x.color) } : undefined}
            />
            <span className="pc-name">{x.label}</span>
            <span>{count(x.rows)}</span>
          </span>
        ))}
      </span>
    </span>
  );
}

/** The ⋯ menu holding Reset, which asks inline before clearing every mark. */
function ResetMenu(props: { disabled: boolean; onReset(): void; onDone(): void }) {
  const { disabled, onReset } = props;
  const [confirming, setConfirming] = useState(false);
  const close = () => {
    setConfirming(false);
    props.onDone();
  };
  if (confirming && !disabled) {
    return (
      <span
        className="reset-confirm"
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          e.stopPropagation();
          close();
        }}
      >
        Clear every mark in this review?
        <button
          type="button"
          className="danger"
          onClick={() => {
            close();
            onReset();
          }}
        >
          Reset
        </button>
        <button type="button" className="quiet" ref={(b) => b?.focus()} onClick={close}>
          Cancel
        </button>
      </span>
    );
  }
  return (
    <Menu
      label="Review actions"
      items={[
        {
          label: "Reset review…",
          title: "Clear every mark in this review",
          danger: true,
          disabled,
          onSelect: () => setConfirming(true),
        },
      ]}
    />
  );
}

/** Notes under the toolbar: stale working copy and changes that left the review. */
export function ReviewNotes({ review, stale }: { review?: ReviewDto; stale: string | null }) {
  const removed = review?.view.kind === "sinceLast" ? review.removedChanges : [];
  return (
    <>
      {stale && (
        <div className="review-note warn">
          The working copy is stale, so edits aren't being picked up. Run{" "}
          <code>jj workspace update-stale</code> in a terminal.
        </div>
      )}
      {removed.length > 0 && (
        <div className="review-note">
          Removed since last session:{" "}
          {removed.map((c) => `${shortId(c)} ${c.description}`).join(", ")}
        </div>
      )}
    </>
  );
}
