import { isAuto, isSubOf, type Question } from "./model";

const isSub = (q: Question) => /[a-z]$/.test(q.id);

function mark(q: Question) {
  if (q.state === "decided") return "✓";
  if (q.state === "dropped") return "–";
  if (q.state === "deferred") return "»";
  if (q.change) return "↺";
  return "○";
}

interface Props {
  questions: Question[];
  /** The question shown. Rows it depends on and rows that depend on it get a stripe. */
  current?: Question;
  /** Questions the agent is updating, dimmed until it's done. */
  updating: string[];
  onPick(id: string): void;
}

/** Every question in queue order, subquestions indented, auto answers badged, with what the
 * shown one depends on and what depends on it. */
export function Rail({ questions, current, updating, onPick }: Props) {
  const needs = current?.depends ?? [];
  const anyAuto = questions.some(isAuto);
  const feeds = (q: Question) =>
    !!current && (q.depends.includes(current.id) || isSubOf(q.id, current.id));
  return (
    <nav className="qq-rail" aria-label="Questions">
      <h3>Questions</h3>
      {questions.map((q) => (
        <button
          key={q.id}
          type="button"
          className={[
            "qq-rail-row",
            q.state,
            isAuto(q) && "auto",
            q.change && q.state === "open" && "reopened",
            isSub(q) && "sub",
            q.id === current?.id && "current",
            needs.includes(q.id) && "needs",
            feeds(q) && "feeds",
            updating.includes(q.id) && "updating",
          ]
            .filter(Boolean)
            .join(" ")}
          title={
            q.dropped
              ? `Dropped: ${q.dropped}`
              : isAuto(q)
                ? `Auto-answered: ${q.answer?.auto}`
                : q.depends.length
                  ? `Depends on ${q.depends.join(", ")}`
                  : undefined
          }
          disabled={q.state === "dropped"}
          onClick={() => onPick(q.id)}
        >
          <span className="qq-rail-mark">{mark(q)}</span>
          <span className="qq-rail-id">{q.id}</span>
          <span className="qq-rail-subject">{q.text.subject}</span>
          {isAuto(q) ? (
            <span className="qq-rail-auto" title="Auto-answered">
              A
            </span>
          ) : (
            <span />
          )}
        </button>
      ))}
      {(current || anyAuto) && (
        <div className="qq-rail-legend">
          {anyAuto && (
            <span>
              <i className="auto" />
              auto-answered
            </span>
          )}
          {current && (
            <>
              <span>
                <i className="needs" />
                {current.id} depends on
              </span>
              <span>
                <i className="feeds" />
                depends on {current.id}
              </span>
            </>
          )}
        </div>
      )}
    </nav>
  );
}
