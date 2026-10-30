import { useState } from "react";
import type { ChangeDto, MarkColor, ReviewDto, TurnDto, View } from "../bindings";
import { turnName, turnTime } from "../turns/turnsStore";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { fixChoices } from "./fixes";
import { MarkBar } from "./MarkBar";
import { type MarkSetup, markVar } from "./marks";
import type { ViewMode } from "./model";
import { ProgressCard, share } from "./ProgressCard";
import type { Filter } from "./rowState";
import { type Progress, progressParts } from "./state";
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
  /** The agent turns and checkpoints on `@`, for the Agent turn view. */
  turns: TurnDto[];
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
  const view: View = review?.view ?? { kind: "combined" };
  const current = view.kind === "change" || view.kind === "sinceLast" ? view.change : undefined;

  function pickKind(kind: View["kind"]) {
    if (!review) return;
    if (kind === "combined") return props.onView({ kind });
    // A fix is opened from its thread.
    if (kind === "fix") return;
    if (kind === "turn") {
      if (props.turns[0]) props.onView({ kind, turns: [props.turns[0].id] });
      return;
    }
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
          <option value="turn" disabled={!props.turns.length && view.kind !== "turn"}>
            Agent turn
          </option>
          <option value="fix" disabled={view.kind !== "fix"}>
            Fix
          </option>
        </select>
      </span>
      {review && (view.kind === "change" || view.kind === "sinceLast") && (
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
      {review && view.kind === "turn" && (
        <span className="quiet-select change-picker">
          <select
            value={view.turns.length === 1 ? view.turns[0] : "run"}
            onChange={(e) => props.onView({ kind: "turn", turns: [Number(e.target.value)] })}
            aria-label="Turn"
          >
            {view.turns.length > 1 && (
              <option value="run" disabled>
                {view.turns.length} snapshots
              </option>
            )}
            {props.turns.map((t) => (
              <option key={t.id} value={t.id}>
                {t.kind === "checkpoint" ? `Checkpoint ${turnTime(t)}` : turnName(t)}
              </option>
            ))}
          </select>
        </span>
      )}
      {review && view.kind === "fix" && (
        <span className="quiet-select change-picker">
          <select
            value={view.fix}
            onChange={(e) => props.onView({ kind: "fix", fix: Number(e.target.value) })}
            aria-label="Fix"
          >
            {fixChoices(review.state.threads, view.fix).map(([id, label]) => (
              <option key={id} value={id}>
                {label}
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

/** Marked rows as one bar split by color, then rows done by their file, with the done share
 * beside it. Hovering shows the lines reviewed and left, and each part's count. */
function ProgressBar({ progress: p, setup }: { progress: Progress; setup: MarkSetup }) {
  const [hover, setHover] = useState(false);
  const parts = progressParts(p, setup);
  return (
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
