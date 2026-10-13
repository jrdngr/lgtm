import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { answerLabel, type Design, unrevised } from "./model";
import { type Busy, runStep, setView, showDesign } from "./store";

/** Shown when no question is open: the decisions, and folding them into the design. */
export function DoneCard({ ws, d, busy }: { ws: WorkspaceName; d: Design; busy?: Busy }) {
  const fresh = unrevised(d);
  const decided = d.queue.questions.filter((q) => q.state === "decided");

  function revise() {
    runStep(
      ws,
      d.slug,
      { kind: "revise", subject: `${d.name}: revise`, open: () => showDesign(ws, d.slug, "docs") },
      (ch) => commands.reviseDesign(ws, d.slug, ch),
    );
  }

  return (
    <div className="qq-done">
      <h2>All {decided.length} questions decided</h2>
      <ul className="qq-decisions">
        {decided.map((q) => (
          <li key={q.id} className={fresh.includes(q) ? "fresh" : undefined}>
            <b>{q.id}</b> {q.text.subject}: {answerLabel(q)}
          </li>
        ))}
      </ul>
      {fresh.length > 0 ? (
        <div className="qq-actions">
          <button type="button" className="primary" disabled={!!busy} onClick={revise}>
            Revise design with {fresh.length} new decision{fresh.length > 1 ? "s" : ""}
          </button>
          <span className="faint">The agent may ask follow-up questions</span>
        </div>
      ) : (
        <div className="qq-actions">
          <span className="faint">The design has every decision.</span>
          <button type="button" onClick={() => setView(ws, "docs", "design.md")}>
            Read the design
          </button>
          <button type="button" className="primary" onClick={() => setView(ws, "files")}>
            Make docs
          </button>
        </div>
      )}
    </div>
  );
}
