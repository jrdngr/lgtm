import { Spinner } from "../ui/Busy";
import { answerLabel, isAuto, type Question } from "./model";

interface Props {
  q: Question;
  /** The question to answer, when `q` is a later one that can only be looked at. */
  blockedBy?: Question;
  /** Questions the running update may change. */
  updating: string[];
  /** The agent is rewriting `q` after its discussion. */
  rewriting: boolean;
  agentBusy: boolean;
  showPrev: boolean;
  onTogglePrev(): void;
  onSeen(): void;
  onReopen(): void;
}

/** What's going on with the shown question, a bar each, above it. */
export function Bars({ q, blockedBy, updating, rewriting, agentBusy, ...p }: Props) {
  const reopenedAuto = q.change?.after === "auto";
  return (
    <>
      {blockedBy && <div className="qq-bar view">Blocked by {blockedBy.id}</div>}
      {q.state === "deferred" && <div className="qq-bar deferred">Deferred</div>}
      {updating.length > 0 && (
        <div className="qq-bar updating">
          <Spinner className="qa-spin" />
          Updating {updating.join(", ")}
        </div>
      )}
      {rewriting && (
        <div className="qq-bar updating">
          <Spinner className="qa-spin" />
          Rewriting {q.id}
        </div>
      )}
      {q.change && q.state === "open" && (
        <div className="qq-bar reopened">
          <span>↺</span>
          <span className="grow">
            {reopenedAuto
              ? "Auto-answer reopened"
              : `Reopened after ${q.change.after === "discussion" ? "discussion" : `${q.change.after} answer`}`}
          </span>
          {!reopenedAuto && (
            <button type="button" className="qq-btn ghost" onClick={p.onTogglePrev}>
              {p.showPrev ? "Hide previous" : "Show previous"} <kbd>p</kbd>
            </button>
          )}
          <button type="button" className="qq-btn ghost" onClick={p.onSeen}>
            Got it
          </button>
        </div>
      )}
      {isAuto(q) && (
        <div className="qq-bar auto">
          <span>Auto-answered</span>
          <span className="why grow">{q.answer?.auto}</span>
        </div>
      )}
      {q.state === "decided" && !isAuto(q) && (
        <div className="qq-bar decided">
          <span className="grow">Decided: {answerLabel(q)}</span>
          <button type="button" className="qq-btn ghost" disabled={agentBusy} onClick={p.onReopen}>
            Change answer <kbd>r</kbd>
          </button>
        </div>
      )}
    </>
  );
}
