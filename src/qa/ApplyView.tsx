import { useEffect, useRef, useState } from "react";
import { Inline } from "../bots/Markdown";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { PipeButton } from "../terminal/PipeButton";
import { edit } from "./actions";
import { answerLabel, applyText, isAuto, openQuestions, type Session } from "./model";
import { type Busy, showQuestion } from "./store";

interface Props {
  ws: WorkspaceName;
  d: Session;
  busy?: Busy;
  /** Moves keyboard focus into the view. */
  focusKey: number;
  onSend(text: string): void;
}

/** The last step: the decisions, auto answers tagged, then sending them to a terminal agent
 * to write. */
export function ApplyView({ ws, d, busy, focusKey, onSend }: Props) {
  const [note, setNote] = useState(d.note);
  const rootRef = useRef<HTMLDivElement>(null);
  const locked = !!busy || d.archived;
  const decided = d.queue.questions.filter((q) => q.state === "decided");
  const open = openQuestions(d).length;
  const deferred = d.queue.questions.filter((q) => q.state === "deferred");

  useEffect(() => setNote(d.note), [d.note]);
  useEffect(() => {
    if (focusKey) rootRef.current?.focus({ preventScroll: true });
  }, [focusKey]);

  const saveNote = () => note !== d.note && edit(ws, d, { kind: "note", note });

  /** Sends the decisions to a terminal agent: the one that asked, or one the user picks. */
  async function go() {
    if (locked) return;
    await saveNote();
    onSend(applyText({ ...d, note }));
  }

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (!mod || e.key !== "Enter") return;
    go();
    e.preventDefault();
  }

  return (
    <div ref={rootRef} className="qa-apply" tabIndex={-1} onKeyDown={onKey}>
      <section className="qq qa-form">
        <span className="qq-label">Decisions</span>
        {decided.length > 0 && (
          <ul className="qa-decisions">
            {decided.map((q) => (
              <li key={q.id}>
                <button type="button" onClick={() => showQuestion(ws, q.id)}>
                  <code>{q.id}</code>
                  <span>
                    <span className="qa-subject">{q.text.subject}</span>{" "}
                    <span className="qa-answer">
                      · <Inline>{answerLabel(q)}</Inline>
                    </span>
                  </span>
                  {isAuto(q) ? <span className="pill auto">auto</span> : <span />}
                  <span className="qa-go">›</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        {!decided.length && !open && <p className="faint">Nothing to decide</p>}
        {open > 0 && <p className="faint">{open} still open</p>}
        {deferred.length > 0 && (
          <p className="faint">
            Deferred, left unimplemented: {deferred.map((q) => q.id).join(", ")}
          </p>
        )}
        {!d.archived && (
          <>
            <label className="qq-label" htmlFor="qa-note">
              Anything else
            </label>
            <textarea
              id="qa-note"
              className="qa-field qa-note"
              placeholder="Extra context for the agent"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              onBlur={saveNote}
            />
            <div className="qq-actions">
              <span className="spacer" />
              <PipeButton background className="qq-btn" disabled={locked} onClick={go} keys="⌘↵">
                {d.sent != null ? "Send Again" : "Answers"}
              </PipeButton>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
