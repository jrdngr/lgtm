import { useEffect, useRef, useState } from "react";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { PipeButton } from "../terminal/PipeButton";
import { submitNextSend } from "../terminal/registry";
import { autoAnswer, savePrompt } from "./actions";
import type { Session } from "./model";
import { ReadFirst } from "./ReadFirst";
import type { Busy } from "./store";

interface Props {
  ws: WorkspaceName;
  /** Unset for a new session. */
  d?: Session;
  busy?: Busy;
  onSend(text: string): void;
}

/** The terminal prompt for asking through an agent the user drives. */
const terminalText = (id: string, prompt: string, attach: string[], auto: boolean) =>
  `${prompt.trim()}${attach.length ? `\n\nRead these first: ${attach.join(", ")}` : ""}\n\nBefore writing anything, ask me every decision you need with lgtm's qa_ask tool (session "${id}"), built by the rules qa_rules returns. Then wait: lgtm sends you prompts to update and discuss questions as I answer, and my decisions at the end.${auto ? " Answer the obvious questions yourself with their `auto` field." : ""}`;

/** What the session is for, files the agent reads first and whether it answers the obvious
 * questions itself, then starting it in a terminal. */
export function PromptView({ ws, d, busy, onSend }: Props) {
  const auto = autoAnswer.use();
  const [prompt, setPrompt] = useState(d?.prompt ?? "");
  const [attach, setAttach] = useState(d?.attach ?? []);
  const ref = useRef<HTMLTextAreaElement>(null);
  const locked = !!busy || !!d?.archived;

  useEffect(() => ref.current?.focus(), []);

  const save = (next = attach) =>
    d || prompt.trim() ? savePrompt(ws, d, prompt, next) : Promise.resolve(undefined);

  async function send() {
    if (!prompt.trim()) return;
    const id = await save();
    if (id) onSend(terminalText(id, prompt, attach, auto));
  }

  function onKey(e: React.KeyboardEvent) {
    if ((isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      submitNextSend();
      send();
    }
  }

  return (
    <div className="qq qa-form qa-prompt" onKeyDown={onKey}>
      <label className="qq-label" htmlFor="qa-prompt">
        Prompt
      </label>
      <textarea
        id="qa-prompt"
        ref={ref}
        className="qa-field qa-prompt-text"
        value={prompt}
        readOnly={locked}
        onChange={(e) => setPrompt(e.target.value)}
        onBlur={() => d && !locked && save()}
      />
      <div className="qq-label">Read first</div>
      <ReadFirst
        ws={ws}
        paths={attach}
        locked={locked}
        onChange={(next) => {
          setAttach(next);
          if (d) save(next);
        }}
      />
      <button
        type="button"
        role="switch"
        aria-checked={auto}
        className="qa-field qa-toggle"
        onClick={() => autoAnswer.set(!auto)}
      >
        <span className="qa-switch" />
        <span>
          <b>Auto-answer obvious questions</b>
          <small>When the other options are non-starters</small>
        </span>
      </button>
      <div className="qq-actions">
        <span className="spacer" />
        <PipeButton className="qq-btn" disabled={!prompt.trim()} onClick={send} keys="⌘↵">
          Start Q&A
        </PipeButton>
      </div>
    </div>
  );
}
