import { useEffect, useRef, useState } from "react";
import { Inline } from "../bots/Markdown";
import { MarkdownView } from "../editor/MarkdownView";
import { openFile } from "../editor/open";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { apply, edit, writePlan } from "./actions";
import { answerLabel, applyText, isAuto, openQuestions, type Session } from "./model";
import { PlanView } from "./PlanView";
import { type Busy, showQuestion } from "./store";

interface Props {
  ws: WorkspaceName;
  d: Session;
  busy?: Busy;
  /** Tool calls of the running plan or apply. */
  feed: string[];
  /** Moves keyboard focus into the view. */
  focusKey: number;
  onSend(text: string): void;
}

/** The last step: the decisions, auto answers tagged, then writing them into @ straight
 * away, through a plan, or from a terminal. */
export function ApplyView({ ws, d, busy, feed, focusKey, onSend }: Props) {
  const [note, setNote] = useState(d.note);
  const rootRef = useRef<HTMLDivElement>(null);
  const locked = !!busy || d.archived;
  const decided = d.queue.questions.filter((q) => q.state === "decided");
  const open = openQuestions(d).length;
  const deferred = d.queue.questions.filter((q) => q.state === "deferred");
  const running = busy?.kind === "apply" || busy?.kind === "plan";

  useEffect(() => setNote(d.note), [d.note]);
  useEffect(() => {
    if (focusKey) rootRef.current?.focus({ preventScroll: true });
  }, [focusKey]);

  const saveNote = () => note !== d.note && edit(ws, d, { kind: "note", note });

  /** Writes the decisions into @, or hands them back to the terminal agent that asked. */
  async function go() {
    if (locked) return;
    await saveNote();
    if (d.fromTerminal) onSend(applyText({ ...d, note }));
    else apply(ws, d);
  }

  async function plan() {
    if (locked) return;
    await saveNote();
    writePlan(ws, d);
  }

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && e.key === "Enter") go();
    else if (e.key === "p" && !mod && !e.altKey && !(e.target instanceof HTMLTextAreaElement))
      plan();
    else return;
    e.preventDefault();
  }

  return (
    <div ref={rootRef} className="qa-apply" tabIndex={-1} onKeyDown={onKey}>
      {running && <Feed feed={feed} />}
      {d.applied && !running && <AppliedCard ws={ws} d={d} />}
      {d.plan && !d.applied ? (
        <PlanView ws={ws} d={d} busy={busy} onApply={go} />
      ) : (
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
                {!d.fromTerminal && (
                  <button
                    type="button"
                    className="qq-btn"
                    disabled={!!busy}
                    onClick={() => onSend(applyText({ ...d, note }))}
                  >
                    Send to terminal
                  </button>
                )}
                <button type="button" className="qq-btn" disabled={locked} onClick={plan}>
                  Plan first
                </button>
                <span className="spacer" />
                <button type="button" className="qq-btn primary" disabled={locked} onClick={go}>
                  {d.fromTerminal ? "Apply in terminal" : "Apply"} <kbd>⌘↵</kbd>
                </button>
              </div>
            </>
          )}
        </section>
      )}
    </div>
  );
}

/** What the running agent has read and written so far. */
function Feed({ feed }: { feed: string[] }) {
  return (
    <ul className="qa-feed">
      {feed.map((t, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the feed only appends
        <li key={i} className={i === feed.length - 1 ? "current" : undefined}>
          {i === feed.length - 1 ? <span className="qa-spin" /> : "✓"} {t}
        </li>
      ))}
    </ul>
  );
}

function AppliedCard({ ws, d }: { ws: WorkspaceName; d: Session }) {
  const a = d.applied;
  if (!a) return null;
  return (
    <section className="qa-card qa-applied">
      <h2>
        Applied to @ · {a.files.length} {a.files.length === 1 ? "file" : "files"}
        {d.archived && <span className="pill green">Archived</span>}
      </h2>
      {a.summary && <MarkdownView text={a.summary} />}
      <div className="qa-files">
        {a.files.map((f) => (
          <button key={f} type="button" className="quiet" onClick={() => openFile(ws, f)}>
            {f}
          </button>
        ))}
      </div>
      {d.archived && (
        <div className="qq-actions">
          <button type="button" onClick={() => edit(ws, d, { kind: "archive", archived: false })}>
            Unarchive
          </button>
        </div>
      )}
    </section>
  );
}
