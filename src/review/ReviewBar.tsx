import { useState } from "react";
import type { ChangeDto, MarkColor, ReviewDto, View } from "../bindings";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { MarkBar } from "./MarkBar";
import { type MarkSetup, markVar } from "./marks";
import type { ViewMode } from "./model";
import { type Filter, type Progress, progressTitle } from "./state";
import { stopWalkthrough, type WalkthroughRun } from "./walkthroughRun";
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
  walkthrough?: WalkthroughRun;
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
      {review && props.walkthrough?.running && (
        <span className="risk-run" title="An agent is writing a walkthrough of the change">
          <span className="risk-note">Walking through · {props.walkthrough.note}</span>
          <button type="button" className="quiet" onClick={() => stopWalkthrough(review.id)}>
            Stop
          </button>
        </span>
      )}
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
 * beside it. */
function ProgressBar({ progress: p, setup }: { progress: Progress; setup: MarkSetup }) {
  return (
    <span className="progress" title={progressTitle(p, setup)}>
      <span className="progress-stack">
        {setup.slots.map(
          (s) =>
            (p.marks[s.color] ?? 0) > 0 && (
              <span
                key={s.color}
                style={{
                  width: `${share(p.marks[s.color] ?? 0, p.changed)}%`,
                  background: markVar(s.color),
                }}
              />
            ),
        )}
        <span className="p-by-file" style={{ width: `${share(p.byFile, p.changed)}%` }} />
      </span>
      <span className="progress-pct">{Math.floor(share(p.done, p.changed))}%</span>
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
