import { useEffect, useRef, useState } from "react";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { GrowingText } from "../ui/GrowingText";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { ActionBar, type Mode } from "./ActionBar";
import {
  decide as decideQuestion,
  discuss as discussQuestion,
  explain as explainOption,
} from "./actions";
import { Bars } from "./Bars";
import { DeferModal } from "./DeferModal";
import { Deps } from "./Deps";
import { Discussion } from "./Discussion";
import {
  autoAnswered,
  browsable,
  countable,
  current,
  dependents,
  head,
  isAuto,
  lastAction,
  type Question,
  type Session,
} from "./model";
import { QuestionCard } from "./QuestionCard";
import { Rail } from "./Rail";
import { act, type Busy, runStep, setError, setView } from "./store";

const isEditable = (t: EventTarget) =>
  t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement;

interface Props {
  ws: WorkspaceName;
  d: Session;
  busy?: Busy;
  streams: Record<string, string>;
  /** Moves keyboard focus into the view. */
  focusKey: number;
}

/** The session's questions, one at a time in the qqueue template. The first open one is
 * answered; the rest, auto answers among them, can be looked at with [ ]. Number keys pick an
 * option, ↵ decides it, and the agent then updates the questions that depend on it. */
export function QuestionsView({ ws, d, busy, streams, focusKey }: Props) {
  const [want, setWant] = useState<string>();
  const [sel, setSel] = useState(0);
  const [mode, setMode] = useState<Mode>();
  const [note, setNote] = useState("");
  const [other, setOther] = useState("");
  const [showPrev, setShowPrev] = useState(false);
  const [deferring, setDeferring] = useState<string[]>();
  const rootRef = useRef<HTMLDivElement>(null);
  const next = head(d);
  /** The questions left to the user, which the count covers. */
  const list = countable(d);
  const all = browsable(d);
  const autos = autoAnswered(d);
  /** With nothing open, the last question shows for looking back. */
  const q = current(d, want) ?? all.at(-1);
  const auto = !!q && isAuto(q);
  const last = lastAction(d);
  /** An open question past the one to answer: it can only be looked at. */
  const ahead = q?.state === "open" && q.id !== next?.id;
  /** The questions the running update may change, which wait for it. */
  const updating = busy?.kind === "answer" && busy.about ? dependents(d, busy.about) : [];
  const updatingIds = updating.map((x) => x.id);
  /** Answers wait for an agent step, except while it updates other questions. */
  const locked =
    d.archived || (!!busy && (busy.kind !== "answer" || (!!q && updatingIds.includes(q.id))));
  /** Another agent step can't start in the same conversation. */
  const agentBusy = !!busy || d.archived;
  const canAnswer = q?.state === "open" && !ahead && !locked;

  // Each question starts on its recommendation, with nothing being written.
  const qid = q?.id;
  useEffect(() => {
    setSel(0);
    setMode((m) => (m === "discuss" ? m : undefined));
    setNote("");
    setOther("");
    setShowPrev(false);
    setDeferring(undefined);
    void qid;
  }, [qid]);

  // A rewrite starts the question on its new recommendation.
  const options = JSON.stringify(q?.text.options);
  useEffect(() => {
    setSel(0);
    void options;
  }, [options]);

  // Resolving the last open question goes on to Apply.
  const done = !next && d.asked && !busy;
  const wasDone = useRef(done);
  useEffect(() => {
    if (done && !wasDone.current) setView(ws, "apply");
    wasDone.current = done;
  }, [done, ws]);

  const focusRoot = () => rootRef.current?.focus({ preventScroll: true });
  useEffect(() => {
    if (focusKey) rootRef.current?.focus({ preventScroll: true });
  }, [focusKey]);

  const id = d.id;

  function show(target: string | undefined) {
    setWant(target);
    focusRoot();
  }

  async function decide(option: number | null, withNote?: string, otherText?: string) {
    if (!q || !canAnswer) return;
    const answer = { option, note: withNote?.trim() || null, other: otherText?.trim() || null };
    if (!(await decideQuestion(ws, d, q.id, answer))) return;
    setWant(undefined);
    setMode(undefined);
    focusRoot();
  }

  function quick(a: Parameters<typeof commands.actOnQuestion>[2]) {
    return act(ws, commands.actOnQuestion(ws, id, a));
  }

  async function undo() {
    if (!last || agentBusy) return;
    const r = await quick({ kind: "undo" });
    if (r.status === "ok") setWant(last.id);
  }

  async function askDefer() {
    if (!q || !canAnswer) return;
    const r = await commands.deferPreview(ws, id, q.id);
    if (r.status === "error") setError(ws, r.error);
    else setDeferring(r.data);
  }

  async function defer() {
    if (!q) return;
    setDeferring(undefined);
    await quick({ kind: "defer", id: q.id });
    setWant(undefined);
    focusRoot();
  }

  function browse(dir: 1 | -1) {
    const i = q ? all.findIndex((x) => x.id === q.id) : all.length;
    const to = all[Math.min(Math.max(i + dir, 0), all.length - 1)];
    if (to) setWant(to.id);
  }

  function reopen() {
    if (!q || agentBusy) return;
    quick({ kind: "reopen", id: q.id });
    focusRoot();
  }

  async function discuss(message: string) {
    if (!q || !(await discussQuestion(ws, d, q.id, message))) return;
    setWant(undefined);
    setMode(undefined);
    focusRoot();
  }

  function explain(option: number) {
    if (!q || agentBusy) return;
    setMode("discuss");
    explainOption(ws, d, q.id, option);
  }

  function reask() {
    if (!q) return;
    const qid = q.id;
    setMode(undefined);
    focusRoot();
    runStep(ws, id, { kind: "reask", about: qid, subject: `${d.title}: ask ${qid} again` }, () =>
      commands.reaskQuestion(ws, id, qid),
    );
  }

  function onKey(e: React.KeyboardEvent) {
    if (isEditable(e.target) || !q || deferring) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod || e.altKey) return;
    // ↵ on a focused button or summary activates it rather than deciding.
    const onControl =
      e.target !== e.currentTarget && !!(e.target as Element).closest("button, summary, a");
    const n = Number(e.key);
    const canTalk = !ahead && q.state !== "dropped" && q.state !== "deferred";
    if (n >= 1 && n <= q.text.options.length && canAnswer) setSel(n - 1);
    else if (e.key === "Enter" && canAnswer && !onControl)
      decide(sel, mode === "note" ? note : undefined);
    else if (e.key === "n" && canAnswer) setMode("note");
    else if (e.key === "o" && canAnswer) setMode("other");
    else if (e.key === "d" && canTalk) setMode("discuss");
    else if (e.key === "s" && canAnswer) askDefer();
    else if (e.key === "u" && last && !agentBusy) undo();
    else if (e.key === "r" && q.state === "decided" && !agentBusy) reopen();
    else if (e.key === "p" && q.change && q.change.after !== "auto") setShowPrev(!showPrev);
    else if ((e.key === "j" || e.key === "ArrowDown") && canAnswer)
      setSel(Math.min(sel + 1, q.text.options.length - 1));
    else if ((e.key === "k" || e.key === "ArrowUp") && canAnswer) setSel(Math.max(sel - 1, 0));
    else if (e.key === "Escape" && q.id !== next?.id && next) show(undefined);
    else if (e.key === "]") browse(1);
    else if (e.key === "[") browse(-1);
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

  if (!d.queue.questions.length)
    return (
      <div className="placeholder">
        {busy?.kind === "ask"
          ? "Questions show here once the agent has read the prompt"
          : d.asked
            ? "Nothing to decide"
            : "No questions yet. Ask from the prompt first."}
      </div>
    );

  const index = q ? list.findIndex((x) => x.id === q.id) : -1;

  return (
    <div ref={rootRef} className="qq" tabIndex={-1} onKeyDown={onKey}>
      <Rail questions={d.queue.questions} current={q} updating={updatingIds} onPick={show} />
      <div className="qq-main">
        {!q ? (
          <div className="placeholder">Nothing to decide</div>
        ) : (
          <>
            {index === 0 && q.id === next?.id && autos.length > 0 && (
              <div className="qq-summary">
                <span className="grow">{autos.length} auto-answered</span>
                <button type="button" className="qq-btn cyan" onClick={() => show(autos[0].id)}>
                  Review them
                </button>
              </div>
            )}
            {!auto && (
              <div className="qq-top">
                <span className="qq-count">
                  Question {index + 1} of {list.length}
                  <span className="qq-dots">
                    {list.map((x) => (
                      <span key={x.id} className={dotClass(x, q, updatingIds)} />
                    ))}
                  </span>
                </span>
              </div>
            )}
            <Bars
              q={q}
              blockedBy={ahead ? next : undefined}
              updating={updatingIds}
              rewriting={busy?.kind === "reask" && busy.about === q.id}
              agentBusy={agentBusy}
              showPrev={showPrev}
              onTogglePrev={() => setShowPrev(!showPrev)}
              onSeen={() => quick({ kind: "seen", id: q.id })}
              onReopen={reopen}
            />
            <QuestionCard
              id={q.id}
              text={q.text}
              before={q.state === "open" ? q.change?.previous : undefined}
              was={q.state === "open" ? q.change?.answer : undefined}
              selected={canAnswer ? sel : undefined}
              decided={q.answer?.option}
              auto={auto}
              locked={!canAnswer}
              className={updatingIds.includes(q.id) ? "locked" : undefined}
              onSelect={setSel}
              onDecide={(i) => decide(i)}
              onExplain={!ahead && !agentBusy && q.state !== "dropped" ? explain : undefined}
              deps={<Deps d={d} q={q} onPick={show} />}
              sources={<Sources ws={ws} q={q} />}
              under={(i) =>
                mode === "note" &&
                i === sel && (
                  <GrowingText
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
              <GrowingText
                autoFocus
                className="qq-input"
                value={other}
                placeholder="None of these: your answer, then ↵ to decide"
                onChange={(e) => setOther(e.target.value)}
                onKeyDown={(e) => onInputKey(e, () => other.trim() && decide(null, "", other))}
              />
            )}
            {mode === "discuss" && !ahead && (
              <Discussion
                key={q.id}
                q={q}
                busy={agentBusy}
                streaming={busy?.kind === "discuss" ? streams[`${id}:${q.id}`] : undefined}
                onSend={discuss}
                onReask={reask}
                onClose={() => {
                  setMode(undefined);
                  focusRoot();
                }}
              />
            )}
            <ActionBar
              q={q}
              next={next}
              ahead={ahead}
              auto={auto}
              canAnswer={canAnswer}
              agentBusy={agentBusy}
              sel={sel}
              last={last}
              browse={all.length > 1}
              onBack={() => show(undefined)}
              onDecide={() => decide(sel, mode === "note" ? note : undefined)}
              onMode={setMode}
              onDefer={askDefer}
              onUndo={undo}
              onReopen={reopen}
            />
            {deferring && (
              <DeferModal
                d={d}
                q={q}
                with={deferring}
                onConfirm={defer}
                onCancel={() => {
                  setDeferring(undefined);
                  focusRoot();
                }}
              />
            )}
          </>
        )}
      </div>
    </div>
  );
}

function dotClass(x: Question, q: Question, updating: string[]) {
  const state =
    x.change && x.state === "open"
      ? "reopened"
      : x.state === "decided"
        ? "decided"
        : x.state === "deferred"
          ? "deferred"
          : "";
  return ["qq-dot", state, x.id === q.id && "current", updating.includes(x.id) && "updating"]
    .filter(Boolean)
    .join(" ");
}

function Sources({ ws, q }: { ws: WorkspaceName; q: Question }) {
  if (!q.sources.length) return null;
  return (
    <div className="qq-sources">
      {q.sources.map((s) => (
        <button key={s} type="button" onClick={() => openFile(ws, s)}>
          {s}
        </button>
      ))}
    </div>
  );
}
