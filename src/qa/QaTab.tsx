import type { IDockviewPanelProps } from "dockview-react";
import { Fragment, useEffect, useRef, useState } from "react";
import { useWorkspace } from "../core/workspace";
import { type SendOutcome, useSendText } from "../review/useSendText";
import { openSettings } from "../settings/open";
import { focusTerminal } from "../terminal/registry";
import { useAgentsOff } from "../tools/agents";
import { copy } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { ApplyView } from "./ApplyView";
import { archiveLabel, edit, remove } from "./actions";
import {
  decisionsText,
  lastDecided,
  openQuestions,
  qqText,
  type Session,
  STEP_NAMES,
  STEPS,
  type Step,
  step,
} from "./model";
import { PromptView } from "./PromptView";
import { QuestionsView } from "./QuestionsView";
import { busyName, setError, setView, useQa } from "./store";
import { stopStep } from "./terminalSteps";
import "./qa.css";
import "./auto.css";
import "./forms.css";

/** Where the session is, each step done, current, reached or to come, joined by a line
 * that fills between reached steps. Steps up to the session's own can be gone back to. */
function Stepper({ d, view }: { d?: Session; view: Step }) {
  const ws = useWorkspace();
  const at = d ? STEPS.indexOf(step(d)) : 0;
  const state = (s: Step, i: number) =>
    s === view ? "current" : i < at ? "done" : i === at ? "reached" : "todo";
  return (
    <ol className="qa-steps">
      {STEPS.map((s, i) => (
        <Fragment key={s}>
          {i > 0 && <li aria-hidden className={`qa-step-line to-${s}${i <= at ? " on" : ""}`} />}
          <li className={`qa-step s-${s} ${state(s, i)}`}>
            <button type="button" disabled={!d || i > at} onClick={() => setView(ws, s)}>
              <span className="qa-dot">{state(s, i) === "done" ? "✓" : i + 1}</span>
              {STEP_NAMES[s]}
            </button>
          </li>
        </Fragment>
      ))}
    </ol>
  );
}

/** The Q&A tab: one session at a time, from its prompt through questions to Apply in
 * terminal. */
export function QaTab({ api }: IDockviewPanelProps) {
  const ws = useWorkspace();
  const state = useQa(ws);
  const agentsOff = useAgentsOff();
  /** Archives the session and closes the tab. */
  async function archive(d: Session) {
    await edit(ws, d, { kind: "archive", archived: true });
    api.close();
  }
  const [focusKey, setFocusKey] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const { sendText, picker } = useSendText(
    ws,
    (e) => setError(ws, e),
    () => rootRef.current?.focus(),
  );
  const d = state.fresh ? undefined : state.sessions.find((x) => x.id === state.current);
  const busy = d ? state.busy[d.id] : undefined;
  const view: Step = d ? (state.view ?? step(d)) : "prompt";

  // Keys go to the view as soon as the tab is picked.
  useEffect(() => {
    if (api.isActive) setFocusKey((n) => n + 1);
    const sub = api.onDidActiveChange(({ isActive }) => isActive && setFocusKey((n) => n + 1));
    return () => sub.dispose();
  }, [api]);

  if (!state.loaded) return null;

  /** Pastes text into a terminal, the one whose agent asked when it still can, and remembers
   * which one took it. */
  function send(text: string) {
    const report = (o: SendOutcome) => {
      if ("error" in o) setError(ws, o.error);
      else if (d) edit(ws, d, { kind: "sent", terminal: o.terminal });
    };
    sendText(text, undefined, report, d?.terminal ?? undefined);
  }

  const q = d && openQuestions(d)[0];
  const prev = d && lastDecided(d);
  return (
    <div ref={rootRef} className="qa" tabIndex={-1}>
      <header className="qa-head">
        <b className="qa-title">{d?.title ?? "New Q&A"}</b>
        {d?.archived && <span className="pill">Archived</span>}
        <Stepper d={d} view={view} />
        <span className="spacer" />
        <Menu
          items={[
            q && {
              label: "Copy question as qq text",
              onSelect: () => copy(qqText(q, prev)),
            },
            d && {
              label: "Copy decisions",
              disabled: !d.queue.questions.some((x) => x.state === "decided"),
              onSelect: () => copy(decisionsText(d)),
            },
            d && {
              label: archiveLabel(d),
              disabled: !!busy,
              onSelect: () =>
                d.archived ? edit(ws, d, { kind: "archive", archived: false }) : archive(d),
            },
            d && {
              label: "Delete",
              danger: true,
              disabled: !!busy,
              onSelect: () => remove(ws, d),
            },
            {
              label: "Edit Q&A prompts…",
              title: "How the agent asks",
              onSelect: () => openSettings(ws, "prompts"),
            },
          ]}
        />
      </header>
      {d && busy && (busy.kind !== "answer" || busy.activity) && (
        <div className="qa-busy">
          <span className="qa-spin" />
          <span>
            {busyName(busy)}
            {busy.activity && ` · ${busy.activity}`}
          </span>
          <span className="spacer" />
          {busy.terminal && (
            <IconButton
              icon="terminal"
              className="pipe-icon"
              label="Terminal"
              onClick={() => busy.terminal && focusTerminal(busy.terminal)}
            />
          )}
          <button type="button" onClick={() => stopStep(d.id)}>
            Stop
          </button>
        </div>
      )}
      {d && d.sent != null && !d.archived && (
        <div className="qa-sent">
          Sent to {d.sent || "a terminal"}
          <span className="spacer" />
          <button type="button" onClick={() => archive(d)}>
            Archive
          </button>
        </div>
      )}
      {state.error && <div className="review-error">{state.error}</div>}
      {agentsOff && (
        <div className="review-error">Claude Code isn't set up, so agents can't run</div>
      )}
      <div className="qa-body">
        {view === "prompt" && (
          <PromptView key={d?.id} ws={ws} d={d} busy={busy} onSend={(t) => sendText(t)} />
        )}
        {d && view === "questions" && (
          <QuestionsView
            key={d.id}
            ws={ws}
            d={d}
            busy={busy}
            focusKey={focusKey}
            jump={state.jump}
          />
        )}
        {d && view === "apply" && (
          <ApplyView key={d.id} ws={ws} d={d} busy={busy} focusKey={focusKey} onSend={send} />
        )}
      </div>
      {picker}
    </div>
  );
}
