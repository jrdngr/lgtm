import type { Question } from "./model";

const isSub = (q: Question) => /[a-z]$/.test(q.id);

function mark(q: Question) {
  if (q.state === "decided") return "✓";
  if (q.state === "dropped") return "–";
  if (q.change) return "✎";
  return "○";
}

/** Every question in queue order, subquestions indented. Open ones dim while the agent
 * updates them. */
export function Rail({
  questions,
  current,
  updating,
  onPick,
}: {
  questions: Question[];
  current?: string;
  updating: boolean;
  onPick(id: string): void;
}) {
  return (
    <nav className="qq-rail">
      {questions.map((q) => (
        <button
          key={q.id}
          type="button"
          className={[
            "qq-rail-row",
            q.state,
            q.change && "changed",
            isSub(q) && "sub",
            q.id === current && "current",
            updating && q.state === "open" && "updating",
          ]
            .filter(Boolean)
            .join(" ")}
          title={q.dropped ? `Dropped: ${q.dropped}` : q.text.subject}
          disabled={q.state === "dropped"}
          onClick={() => onPick(q.id)}
        >
          <span className="qq-rail-mark">{mark(q)}</span>
          <span className="qq-rail-id">{q.id}</span>
          <span className="qq-rail-subject">{q.text.subject}</span>
        </button>
      ))}
    </nav>
  );
}
