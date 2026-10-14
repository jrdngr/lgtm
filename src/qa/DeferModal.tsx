import { useEffect, useRef } from "react";
import type { Question, Session } from "./model";

interface Props {
  d: Session;
  q: Question;
  /** The questions deferred with it, in queue order. */
  with: string[];
  onConfirm(): void;
  onCancel(): void;
}

/** Confirms deferring `q` and the questions that depend on it. ↵ defers, esc cancels. */
export function DeferModal({ d, q, with: rest, onConfirm, onCancel }: Props) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => ref.current?.focus(), []);
  const all = [q, ...rest.map((id) => d.queue.questions.find((x) => x.id === id))].filter(
    (x): x is Question => !!x,
  );
  const ids = new Set(all.map((x) => x.id));

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "Escape") onCancel();
    else if (e.key === "Enter") onConfirm();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  return (
    <div
      className="qq-scrim"
      onKeyDown={onKey}
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className="qq-modal" role="dialog" aria-labelledby="qq-defer-title">
        <h3 id="qq-defer-title">
          {rest.length ? `Defer ${q.id} and what depends on it?` : `Defer ${q.id}?`}
        </h3>
        <p>Apply leaves {rest.length ? "these" : "it"} unimplemented.</p>
        <ul>
          {all.map((x) => {
            const via = x.id === q.id ? [] : x.depends.filter((id) => ids.has(id));
            return (
              <li key={x.id}>
                <code>{x.id}</code>
                <span>
                  {x.text.subject}
                  {via.length > 0 && <small>Depends on {via.join(", ")}</small>}
                </span>
              </li>
            );
          })}
        </ul>
        <div className="qq-modal-row">
          <button type="button" className="qq-btn" onClick={onCancel}>
            Cancel <kbd>esc</kbd>
          </button>
          <button ref={ref} type="button" className="qq-btn defer" onClick={onConfirm}>
            {all.length > 1 ? `Defer ${all.length} questions` : `Defer ${q.id}`} <kbd>↵</kbd>
          </button>
        </div>
      </div>
    </div>
  );
}
