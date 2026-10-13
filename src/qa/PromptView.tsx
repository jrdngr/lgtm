import { useEffect, useRef, useState } from "react";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { ask, savePrompt } from "./actions";
import type { Session } from "./model";
import type { Busy } from "./store";

interface Props {
  ws: WorkspaceName;
  /** Unset for a new session. */
  d?: Session;
  busy?: Busy;
  onSend(text: string): void;
}

/** The terminal prompt for asking through an agent the user drives. */
const terminalText = (id: string, prompt: string) =>
  `${prompt.trim()}\n\nBefore writing anything, ask me every decision you need with lgtm's qa_ask tool (session "${id}"), then wait and read my answers with qa_answers.`;

/** What the session is for and files the agent reads first, then asking. */
export function PromptView({ ws, d, busy, onSend }: Props) {
  const [prompt, setPrompt] = useState(d?.prompt ?? "");
  const [attach, setAttach] = useState(d?.attach ?? []);
  const [path, setPath] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  const locked = !!busy || !!d?.archived;
  const asked = !!d?.queue.questions.length;

  useEffect(() => ref.current?.focus(), []);

  const save = (next = attach) =>
    d || prompt.trim() ? savePrompt(ws, d, prompt, next) : Promise.resolve(undefined);

  async function go() {
    if (locked || !prompt.trim()) return;
    const id = await save();
    if (id) ask(ws, id, prompt.split("\n")[0].slice(0, 60));
  }

  async function send() {
    if (!prompt.trim()) return;
    const id = await save();
    if (id) onSend(terminalText(id, prompt));
  }

  function addPath() {
    const p = path.trim();
    if (!p || attach.includes(p)) return;
    const next = [...attach, p];
    setAttach(next);
    setPath("");
    if (d) save(next);
  }

  function onKey(e: React.KeyboardEvent) {
    if ((isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      go();
    }
  }

  return (
    <div className="qa-prompt" onKeyDown={onKey}>
      <label className="qq-label" htmlFor="qa-prompt">
        Prompt
      </label>
      <textarea
        id="qa-prompt"
        ref={ref}
        className="qa-prompt-text"
        value={prompt}
        readOnly={!!d?.archived}
        placeholder="What you want done. The agent asks every decision it needs before it writes anything."
        onChange={(e) => setPrompt(e.target.value)}
        onBlur={() => d && save()}
      />
      <div className="qq-label">Read first</div>
      <div className="qa-attach">
        {attach.map((a) => (
          <span key={a} className="qa-chip">
            {a}
            <button
              type="button"
              className="quiet"
              aria-label={`Remove ${a}`}
              onClick={() => {
                const next = attach.filter((x) => x !== a);
                setAttach(next);
                if (d) save(next);
              }}
            >
              ×
            </button>
          </span>
        ))}
        <input
          value={path}
          placeholder="A file or folder in the repo, then ↵"
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addPath();
            }
          }}
        />
      </div>
      <div className="qq-actions">
        <button type="button" disabled={!prompt.trim()} onClick={send}>
          Send to terminal
        </button>
        <span className="spacer" />
        {asked && <span className="faint">Asking again replaces the questions and answers</span>}
        <button type="button" className="primary" disabled={locked || !prompt.trim()} onClick={go}>
          {asked ? "Ask again" : "Ask me"} <kbd>⌘↵</kbd>
        </button>
      </div>
    </div>
  );
}
