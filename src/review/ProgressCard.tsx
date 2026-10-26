import { createPortal } from "react-dom";
import { markVar } from "./marks";
import type { Progress, ProgressPart } from "./state";
import "./ProgressCard.css";

export const share = (n: number, of: number) => (of > 0 ? (n / of) * 100 : 0);

const count = (n: number) => n.toLocaleString("en-US");

/** Lines reviewed and left, and each mark's count, shown on hovering a review's progress.
 * Below its parent, or fixed `at` a place in the window when a parent would clip it. */
export function ProgressCard({
  p,
  parts,
  at,
}: {
  p: Progress;
  parts: ProgressPart[];
  at?: { top: number; right: number };
}) {
  const bar = [...parts.filter((x) => x.done), ...parts.filter((x) => !x.done)];
  const card = (
    <span className="progress-card" style={at && { position: "fixed", ...at }}>
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
  // Fixed cards go to the body, so no pane draws over them.
  return at ? createPortal(card, document.body) : card;
}
