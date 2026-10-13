import { useState } from "react";
import type { ChangeDto, ReviewDto, View } from "../bindings";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import type { ViewMode } from "./model";
import { FILTERS, type Filter, type Progress } from "./state";
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

const FILTER_LABEL: Record<Filter, string> = {
  all: "All lines",
  unreviewed: "Unreviewed",
  reviewed: "Reviewed",
  skipped: "Skipped",
  flagged: "Flagged",
};

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
  onReset(): void;
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
          {FILTERS.map((f) => (
            <option key={f} value={f}>
              {FILTER_LABEL[f]}
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
      {review && <ProgressBar progress={props.progress} />}
      <ResetMenu disabled={!review} onReset={props.onReset} />
    </div>
  );
}

const share = (n: number, of: number) => (of > 0 ? (n / of) * 100 : 0);

/** Reviewed, skipped, and flagged rows as one bar, with the reviewed share beside it. */
function ProgressBar({ progress: p }: { progress: Progress }) {
  const title = `${p.reviewed} reviewed, ${p.skipped} skipped, ${p.flagged} flagged, ${p.unreviewed} left`;
  return (
    <span className="progress" title={title}>
      <span className="progress-stack">
        <span className="p-reviewed" style={{ width: `${share(p.reviewed, p.changed)}%` }} />
        <span className="p-skipped" style={{ width: `${share(p.skipped, p.changed)}%` }} />
        <span className="p-flagged" style={{ width: `${share(p.flagged, p.changed)}%` }} />
      </span>
      <span className="progress-pct">{Math.floor(share(p.reviewed, p.changed))}%</span>
    </span>
  );
}

/** The ⋯ menu holding Reset, which asks inline before clearing every mark. */
function ResetMenu({ disabled, onReset }: { disabled: boolean; onReset(): void }) {
  const [confirming, setConfirming] = useState(false);
  if (confirming && !disabled) {
    return (
      <span
        className="reset-confirm"
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          e.stopPropagation();
          setConfirming(false);
        }}
      >
        Clear every mark in this review?
        <button
          type="button"
          className="danger"
          onClick={() => {
            setConfirming(false);
            onReset();
          }}
        >
          Reset
        </button>
        <button
          type="button"
          className="quiet"
          ref={(b) => b?.focus()}
          onClick={() => setConfirming(false)}
        >
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
