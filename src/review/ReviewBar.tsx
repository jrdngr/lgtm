import type { ChangeDto, ReviewDto, View } from "../bindings";
import type { WorkspaceName } from "../workspaces/context";
import type { ViewMode } from "./model";
import { RevisionsPanel } from "./RevisionsPanel";
import { RevLabel } from "./RevLabel";
import { FILTERS, type Filter, type Progress } from "./state";
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
  workspace: WorkspaceName;
  review?: ReviewDto;
  loading: boolean;
  revset: string;
  /** Loads a revset as the review. Returns the error message when it fails. */
  onLoad(revset: string): Promise<string | undefined>;
  /** Whether the revisions panel is open. */
  panel: boolean;
  onPanel(open: boolean): void;
  onView(view: View): void;
  mode: ViewMode;
  onMode(mode: ViewMode): void;
  filter: Filter;
  onFilter(filter: Filter): void;
  progress: Progress;
  onReset(): void;
}

export function ReviewBar(props: Props) {
  const { review, progress } = props;
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
      <button
        type="button"
        className="rev-chip"
        onClick={() => props.onPanel(true)}
        title="Revisions (r): pick what to review, edit, add and squash commits"
      >
        {props.loading || !review ? (
          "Loading…"
        ) : (
          <RevLabel revset={props.revset} changes={review.changes} />
        )}
      </button>
      {props.panel && (
        <RevisionsPanel
          workspace={props.workspace}
          revset={props.revset}
          head={review?.changes.at(-1)?.commitId}
          onLoad={props.onLoad}
          onClose={() => props.onPanel(false)}
        />
      )}
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
      {review && view.kind !== "combined" && (
        <select
          className="change-picker"
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
      )}
      <select
        value={props.filter}
        onChange={(e) => props.onFilter(e.target.value as Filter)}
        aria-label="Line filter"
      >
        {FILTERS.map((f) => (
          <option key={f} value={f}>
            {f[0].toUpperCase() + f.slice(1)}
          </option>
        ))}
      </select>
      <button
        type="button"
        onClick={() => props.onMode(props.mode === "unified" ? "split" : "unified")}
      >
        {props.mode === "unified" ? "Split" : "Unified"}
      </button>
      <span className="spacer" />
      {review && (
        <span className="progress" title="Changed rows by state">
          <span className="p-reviewed">{progress.reviewed} reviewed</span>
          <span className="p-skipped">{progress.skipped} skipped</span>
          <span className="p-flagged">{progress.flagged} flagged</span>
          <span>{progress.unreviewed} left</span>
        </span>
      )}
      <button type="button" disabled={!review} onClick={props.onReset}>
        Reset marks for this review
      </button>
    </div>
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
