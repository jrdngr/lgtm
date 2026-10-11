import { useWorkspace } from "../workspaces/context";
import { rowLabel } from "./comments";
import { useCurrentReview } from "./current";
import { stopRisks, useRiskRun } from "./riskRun";
import "./comments.css";

const SOURCE = { headless: "Highlight risks", terminal: "a terminal agent" } as const;

/** The current review's hotspots, worst first. Click one to jump to it. */
export function HotspotsPanel() {
  const review = useCurrentReview(useWorkspace());
  const run = useRiskRun(review?.id);
  if (!review) return <div className="placeholder">No review open</div>;
  const { hotspots } = review;
  return (
    <div className="comment-list hotspot-list">
      <div className="tree-total">
        {run?.running ? (
          <>
            <span className="risk-note">{run.note}</span>
            <span className="spacer" />
            <button type="button" onClick={() => stopRisks(review.id)}>
              Stop
            </button>
          </>
        ) : (
          <>
            <span className={run?.error ? "error" : undefined}>
              {run?.error ?? `${hotspots.length} ${hotspots.length === 1 ? "hotspot" : "hotspots"}`}
            </span>
            <span className="spacer" />
            <button type="button" onClick={review.findRisks} title="h in the review">
              Highlight risks
            </button>
          </>
        )}
      </div>
      {hotspots.length === 0 && !run?.running && (
        <div className="placeholder">
          No hotspots. Highlight risks has an agent find the parts that most need a careful look.
        </div>
      )}
      <ul>
        {hotspots.map((h) => (
          <li key={h.id}>
            <button
              type="button"
              onClick={() =>
                h.rows ? review.jumpToRow(h.file, h.rows.start) : review.jump(h.file)
              }
              title={`${h.path}, from ${SOURCE[h.source]}`}
            >
              <span className="comment-where">
                <span className={`risk-dot risk-${h.level}`} />
                {rowLabel(review.files, h.file, h.path, h.rows?.start)}
                {!h.rows && <span className="comment-outdated">code changed</span>}
              </span>
              <span className="comment-preview">{h.reason}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
