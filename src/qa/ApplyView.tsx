import { useEffect, useRef, useState } from "react";
import { Inline } from "../bots/Markdown";
import { MarkdownView } from "../editor/MarkdownView";
import { openFile } from "../editor/open";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { apply, edit, writePlan } from "./actions";
import { answerLabel, applyText, openQuestions, type Session } from "./model";
import { PlanView } from "./PlanView";
import type { Busy } from "./store";

/** Decisions listed before the rest fold under "n more". */
const SHOWN = 3;

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

/** The last step: the decisions, then writing them into @ straight away, through a plan, or
 * from a terminal. */
export function ApplyView({ ws, d, busy, feed, focusKey, onSend }: Props) {
  const [all, setAll] = useState(false);
  const [note, setNote] = useState(d.note);
  const rootRef = useRef<HTMLDivElement>(null);
  const locked = !!busy || d.archived;
  const decided = d.queue.questions.filter((q) => q.state === "decided");
  const open = openQuestions(d).length;
  const running = busy?.kind === "apply" || busy?.kind === "plan";

  useEffect(() => setNote(d.note), [d.note]);
  useEffect(() => {
    if (focusKey) rootRef.current?.focus({ preventScroll: true });
  }, [focusKey]);

  const saveNote = () => note !== d.note && edit(ws, d, { kind: "note", note });

  async function go() {
    if (locked) return;
    await saveNote();
    apply(ws, d);
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
        <section className="qa-card">
          <h2>
            {open ? `${decided.length} decided, ${open} open` : `All ${decided.length} decided`}
          </h2>
          <ul className="qa-decisions">
            {(all ? decided : decided.slice(0, SHOWN)).map((q) => (
              <li key={q.id}>
                <b>{q.id}</b> <Inline>{answerLabel(q)}</Inline>
              </li>
            ))}
          </ul>
          {decided.length > SHOWN && (
            <button type="button" className="quiet" onClick={() => setAll(!all)}>
              {all ? "Fewer ▴" : `${decided.length - SHOWN} more ▸`}
            </button>
          )}
          {!d.archived && (
            <>
              <label className="qq-label" htmlFor="qa-note">
                Anything to add for the write? (optional)
              </label>
              <textarea
                id="qa-note"
                className="qq-input"
                rows={2}
                value={note}
                onChange={(e) => setNote(e.target.value)}
                onBlur={saveNote}
              />
              <div className="qq-actions">
                <button
                  type="button"
                  disabled={!!busy}
                  onClick={() => onSend(applyText({ ...d, note }))}
                >
                  Send to terminal
                </button>
                <span className="spacer" />
                <button type="button" disabled={locked} onClick={plan}>
                  Write plan <kbd>p</kbd>
                </button>
                <button type="button" className="primary" disabled={locked} onClick={go}>
                  Apply to @ <kbd>⌘↵</kbd>
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
