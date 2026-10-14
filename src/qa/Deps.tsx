import { answerLabel, dependents, type Question, type Session } from "./model";

function Chip({ q, kind, onPick }: { q: Question; kind: string; onPick(id: string): void }) {
  const done = q.state === "decided";
  return (
    <button type="button" className={`qq-dep ${kind}`} onClick={() => onPick(q.id)}>
      <span className={`qq-dep-state ${done ? "done" : ""}`}>{done ? "✓" : "○"}</span>
      <code>{q.id}</code>
      {q.text.subject}
      {done && <span className="qq-dep-answer"> · {answerLabel(q)}</span>}
    </button>
  );
}

/** The questions `q` depends on and the ones that depend on it, as chips that show them. */
export function Deps({ d, q, onPick }: { d: Session; q: Question; onPick(id: string): void }) {
  const byId = (id: string) => d.queue.questions.find((x) => x.id === id);
  const needs = q.depends.map(byId).filter((x): x is Question => !!x && x.state !== "dropped");
  const feeds = dependents(d, q.id);
  if (!needs.length && !feeds.length) return null;
  return (
    <div className="qq-deps">
      {needs.length > 0 && <span className="qq-deps-label">Depends on</span>}
      {needs.map((x) => (
        <Chip key={x.id} q={x} kind="needs" onPick={onPick} />
      ))}
      {feeds.length > 0 && (
        <span className={`qq-deps-label${needs.length ? " gap" : ""}`}>Affects</span>
      )}
      {feeds.map((x) => (
        <Chip key={x.id} q={x} kind="feeds" onPick={onPick} />
      ))}
    </div>
  );
}
