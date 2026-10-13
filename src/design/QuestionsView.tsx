import { useEffect, useRef, useState } from "react";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { decide as decideQuestion } from "./actions";
import { Discussion } from "./Discussion";
import { DoneCard } from "./DoneCard";
import { answerLabel, countable, current, type Design, lastDecided, type Question } from "./model";
import { QuestionCard } from "./QuestionCard";
import { Rail } from "./Rail";
import { act, type Busy, runStep, showDesign } from "./store";

const isEditable = (t: EventTarget) =>
  t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement;

type Mode = "note" | "other" | "discuss";

interface Props {
  ws: WorkspaceName;
  d: Design;
  busy?: Busy;
  streams: Record<string, string>;
  /** Moves keyboard focus into the view. */
  focusKey: number;
}

/** The design's open questions, one at a time in the qqueue template. Number keys pick an
 * option, ↵ decides it, and the agent then updates the questions still open. */
export function QuestionsView({ ws, d, busy, streams, focusKey }: Props) {
  const [want, setWant] = useState<string>();
  const [sel, setSel] = useState(0);
  const [mode, setMode] = useState<Mode>();
  const [note, setNote] = useState("");
  const [other, setOther] = useState("");
  const [showPrev, setShowPrev] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const q = current(d, want);
  const wanted = d.queue.questions.find((x) => x.id === want);
  const list = countable(d);
  const prev = lastDecided(d);
  /** The agent is changing the queue: answers wait for it. */
  const locked = !!busy;
  const updating = busy?.kind === "answer" || busy?.kind === "reask" || busy?.kind === "draft";

  // Each question starts on its recommendation, with nothing being written.
  const qid = q?.id;
  useEffect(() => {
    setSel(0);
    setMode((m) => (m === "discuss" ? m : undefined));
    setNote("");
    setOther("");
    setShowPrev(false);
    void qid;
  }, [qid]);

  const focusRoot = () => rootRef.current?.focus({ preventScroll: true });
  useEffect(() => {
    if (focusKey) rootRef.current?.focus({ preventScroll: true });
  }, [focusKey]);

  const slug = d.slug;
  const open = () => showDesign(ws, slug, "questions");

  async function decide(option: number | null, withNote?: string, otherText?: string) {
    if (!q || locked || q.state !== "open") return;
    const answer = { option, note: withNote?.trim() || null, other: otherText?.trim() || null };
    if (!(await decideQuestion(ws, d, q.id, answer))) return;
    setWant(undefined);
    setMode(undefined);
    focusRoot();
  }

  function quick(a: Parameters<typeof commands.actOnQuestion>[2]) {
    return act(ws, commands.actOnQuestion(ws, slug, a));
  }

  async function undo() {
    if (!prev || locked) return;
    const r = await quick({ kind: "undo" });
    if (r.status === "ok") setWant(prev.id);
  }

  function browse(dir: 1 | -1) {
    const i = q ? list.findIndex((x) => x.id === q.id) : list.length;
    const next = list[Math.min(Math.max(i + dir, 0), list.length - 1)];
    if (next) setWant(next.id);
  }

  function discuss(message: string) {
    if (!q) return;
    const id = q.id;
    runStep(
      ws,
      slug,
      { kind: "discuss", about: id, subject: `${d.name}: ${id}`, streams: true, open },
      (ch) => commands.discussQuestion(ws, slug, id, message, ch),
    );
  }

  function reask() {
    if (!q) return;
    const id = q.id;
    setMode(undefined);
    focusRoot();
    runStep(
      ws,
      slug,
      { kind: "reask", about: id, subject: `${d.name}: ask ${id} again`, open },
      () => commands.reaskQuestion(ws, slug, id),
    );
  }

  function onKey(e: React.KeyboardEvent) {
    if (isEditable(e.target) || !q) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod || e.altKey) return;
    const isOpen = q.state === "open";
    const n = Number(e.key);
    if (n >= 1 && n <= q.text.options.length && isOpen && !locked) setSel(n - 1);
    else if (e.key === "Enter" && isOpen) decide(sel);
    else if (e.key === "n" && isOpen && !locked) setMode("note");
    else if (e.key === "o" && isOpen && !locked) setMode("other");
    else if (e.key === "d") setMode("discuss");
    else if (e.key === "s" && isOpen) {
      quick({ kind: "later", id: q.id });
      setWant(undefined);
    } else if (e.key === "u" && prev && !locked) undo();
    else if (e.key === "r" && q.state === "decided" && !locked) quick({ kind: "reopen", id: q.id });
    else if (e.key === "p" && q.change) setShowPrev(!showPrev);
    else if (e.key === "j" || e.key === "ArrowDown") browse(1);
    else if (e.key === "k" || e.key === "ArrowUp") browse(-1);
    else return;
    e.preventDefault();
  }

  function onInputKey(e: React.KeyboardEvent, submit: () => void) {
    if (e.key === "Escape") {
      setMode(undefined);
      focusRoot();
    } else if (e.key === "Enter" && !e.shiftKey) submit();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  if (d.queueError)
    return <div className="review-error">Can't read questions.json: {d.queueError}</div>;
  if (!d.queue.questions.length)
    return (
      <div className="placeholder">
        {busy?.kind === "draft"
          ? "Questions show here once the draft is written"
          : "No questions yet. Draft the design from its brief first."}
      </div>
    );

  const index = q ? list.findIndex((x) => x.id === q.id) : -1;
  const hints: [string, string][] = [];
  if (q?.state === "open" && !locked) {
    const n = q.text.options.length;
    hints.push([n > 1 ? `1–${n}` : "1", "select"], ["↵", "decide"], ["n", "note"], ["o", "other"]);
  }
  if (q) hints.push(["d", "discuss"]);
  if (q?.state === "open") hints.push(["s", "later"]);
  if (q?.state === "decided" && !locked) hints.push(["r", "change answer"]);
  if (prev && !locked) hints.push(["u", "undo"]);
  if (q?.change) hints.push(["p", showPrev ? "hide previous" : "show previous"]);
  if (list.length > 1) hints.push(["j k", "browse"]);

  return (
    <div ref={rootRef} className="qq" tabIndex={-1} onKeyDown={onKey}>
      <Rail
        questions={d.queue.questions}
        current={q?.id}
        updating={updating}
        onPick={(id) => {
          setWant(id);
          focusRoot();
        }}
      />
      <div className="qq-main">
        {!q ? (
          <DoneCard ws={ws} d={d} busy={busy} />
        ) : (
          <>
            {prev && prev.id !== q.id && (
              <div className="qq-prev">
                ✅ {prev.id}: {answerLabel(prev)}
              </div>
            )}
            {wanted?.state === "dropped" && (
              <div className="qq-bar dropped">
                {wanted.id} dropped: {wanted.dropped}
              </div>
            )}
            <div className="qq-count">
              Question {index + 1} of {list.length}
              <span className="qq-dots">
                {list.map((x) => (
                  <span
                    key={x.id}
                    className={`qq-dot ${x.state}${x.id === q.id ? " current" : ""}`}
                  />
                ))}
              </span>
            </div>
            {updating && busy?.about && (
              <div className="qq-bar updating">
                <span className="design-spin" />
                {busy.kind === "reask"
                  ? `Rewriting ${busy.about} after your discussion`
                  : busy.kind === "draft"
                    ? "Drafting the design"
                    : `Updating the open questions after your answer to ${busy.about}`}
              </div>
            )}
            {q.change && !updating && (
              <div className="qq-bar changed">
                Changed after {q.change.after === "discussion" ? "your discussion" : q.change.after}
                : {q.change.what}
                <span className="spacer" />
                <button type="button" className="quiet" onClick={() => setShowPrev(!showPrev)}>
                  {showPrev ? "Hide previous" : "Show previous"} <kbd>p</kbd>
                </button>
                <button
                  type="button"
                  className="quiet"
                  onClick={() => quick({ kind: "seen", id: q.id })}
                >
                  Got it
                </button>
              </div>
            )}
            {q.state === "decided" && (
              <div className="qq-bar decided">
                Decided: {answerLabel(q)}
                <span className="spacer" />
                <button
                  type="button"
                  className="quiet"
                  disabled={locked}
                  onClick={() => quick({ kind: "reopen", id: q.id })}
                >
                  Change answer <kbd>r</kbd>
                </button>
              </div>
            )}
            <QuestionCard
              id={q.id}
              text={q.text}
              before={q.change?.previous}
              selected={q.state === "open" ? sel : undefined}
              decided={q.answer?.option}
              locked={locked || q.state !== "open"}
              className={updating && q.state === "open" ? "locked" : undefined}
              onSelect={setSel}
              onDecide={(i) => decide(i)}
              under={(i) =>
                mode === "note" &&
                i === sel && (
                  <input
                    // biome-ignore lint/a11y/noAutofocus: the note is what `n` asked for
                    autoFocus
                    className="qq-input"
                    value={note}
                    placeholder={`A note with option ${i + 1}, then ↵ to decide`}
                    onChange={(e) => setNote(e.target.value)}
                    onKeyDown={(e) => onInputKey(e, () => decide(sel, note))}
                  />
                )
              }
            />
            {showPrev && q.change && (
              <QuestionCard id={q.id} text={q.change.previous} className="previous" locked />
            )}
            {mode === "other" && (
              <input
                // biome-ignore lint/a11y/noAutofocus: the answer is what `o` asked for
                autoFocus
                className="qq-input"
                value={other}
                placeholder="None of these: your answer, then ↵ to decide"
                onChange={(e) => setOther(e.target.value)}
                onKeyDown={(e) => onInputKey(e, () => other.trim() && decide(null, "", other))}
              />
            )}
            {q.state === "open" && (
              <div className="qq-actions">
                <button
                  type="button"
                  className="primary"
                  disabled={locked}
                  onClick={() => decide(sel, mode === "note" ? note : undefined)}
                >
                  {sel === 0 ? "Go with 1" : `Go with ${sel + 1}`} <kbd>↵</kbd>
                </button>
                <button type="button" disabled={locked} onClick={() => setMode("note")}>
                  Note
                </button>
                <button type="button" disabled={locked} onClick={() => setMode("other")}>
                  Other
                </button>
                <button type="button" onClick={() => setMode("discuss")}>
                  Discuss
                </button>
                <button
                  type="button"
                  className="quiet"
                  onClick={() => {
                    quick({ kind: "later", id: q.id });
                    setWant(undefined);
                  }}
                >
                  Later
                </button>
              </div>
            )}
            {mode === "discuss" && (
              <Discussion
                key={q.id}
                q={q}
                busy={!!busy}
                streaming={busy?.kind === "discuss" ? streams[`${slug}:${q.id}`] : undefined}
                onSend={discuss}
                onReask={reask}
                onClose={() => {
                  setMode(undefined);
                  focusRoot();
                }}
              />
            )}
            <Sources ws={ws} q={q} />
            <div className="qq-hints">
              {hints.map(([k, label]) => (
                <span key={label}>
                  <kbd>{k}</kbd> {label}
                </span>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function Sources({ ws, q }: { ws: WorkspaceName; q: Question }) {
  if (!q.sources.length) return null;
  return (
    <details className="qq-sources">
      <summary>Sources ({q.sources.length})</summary>
      {q.sources.map((s) => (
        <button key={s} type="button" className="quiet" onClick={() => openFile(ws, s)}>
          {s}
        </button>
      ))}
    </details>
  );
}
