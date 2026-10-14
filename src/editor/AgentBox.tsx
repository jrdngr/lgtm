import { useEffect, useRef, useState } from "react";
import type { SelectedLines } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { Busy } from "../ui/Busy";
import { IconButton } from "../ui/icons";
import { isMac } from "../workbench/keys";
import { baseName } from "./open";
import "./agentBox.css";

/** How the tab took a change: loaded it, held it behind unsaved edits, or had nothing new. */
export type Reload = "loaded" | "held" | "same";

export interface Turn {
  ask: string;
  change: boolean;
  reply: string;
  failed?: boolean;
  /** A change that ran to the end. */
  changed?: boolean;
}

/** An agent box's conversation, which lives as long as the box. */
export interface Box {
  key: string;
  /** Change, or Ask: how it opened, then what was last sent. */
  change: boolean;
  /** `file.rs:8-13`. */
  where: string;
  lines: SelectedLines;
  turns: Turn[];
  session: string | null;
  running?: boolean;
  /** How the tab took the last change. */
  reload?: Reload;
}

const CHANGE_KEY = isMac ? "⌘↵" : "Ctrl+↵";

function Chat({ box }: { box: Box }) {
  if (!box.turns.length) return null;
  const last = box.turns.length - 1;
  return (
    <div className="agent-chat">
      {box.turns.map((t, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: turns are append-only
        <div key={i} className="agent-turn">
          <div className="agent-msg">
            <span className="agent-who">You</span>
            <span className="agent-text">{t.ask}</span>
          </div>
          <div className="agent-msg agent-reply">
            <span className="agent-who">{t.changed ? "Agent · changed the code" : "Agent"}</span>
            {box.running && i === last && !t.reply ? (
              <Busy className="agent-thinking">
                {t.change ? "Changing the code…" : "Thinking…"}
              </Busy>
            ) : t.failed ? (
              <span className="agent-text agent-error">{t.reply}</span>
            ) : (
              <div className="agent-text">
                <Markdown>{t.reply}</Markdown>
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function Reloaded({ how, name }: { how: Reload; name: string }) {
  if (how === "same") return <div className="agent-reloaded">{name} is unchanged.</div>;
  return (
    <div className="agent-reloaded">
      <b>✓ Saved</b>
      {how === "loaded"
        ? `${name} reloaded.`
        : `${name} changed on disk. Load it from the bar above.`}
    </div>
  );
}

interface Props {
  box: Box;
  send(message: string, change: boolean): void;
  stop(): void;
  close(): void;
}

/** The box under a selection in an editor tab that asks the agent about it, or has it change
 * the code. ↵ sends as the box's kind, and ⌘↵ always as a change. */
export function AgentBox({ box, send, stop, close }: Props) {
  const [draft, setDraft] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    root.current?.scrollIntoView({ block: "nearest" });
  }, []);
  useEffect(() => {
    if (!box.running) input.current?.focus();
  }, [box.running]);

  function submit(change: boolean) {
    if (!draft.trim()) return;
    send(draft, change);
    setDraft("");
  }

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Enter" && mod) submit(true);
    else if (e.key === "Enter" && !e.shiftKey) submit(box.change);
    else return;
    e.preventDefault();
  }

  function onBoxKey(e: React.KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    close();
  }

  const enter = <kbd>↵</kbd>;
  return (
    <div ref={root} className={`agent-box${box.change ? " change" : ""}`} onKeyDown={onBoxKey}>
      <div className="agent-bar">
        <span className="agent-kind">{box.change ? "Change" : "Ask"}</span>
        <span className="agent-where">{box.where}</span>
        <span className="agent-space" />
        {box.running && (
          <button type="button" onClick={stop}>
            Stop
          </button>
        )}
        <IconButton icon="close" label="Close (Esc)" onClick={close} />
      </div>
      <div className="agent-body">
        <Chat box={box} />
        {box.reload && <Reloaded how={box.reload} name={baseName(box.lines.path)} />}
        {!box.running && (
          <div className="agent-ask">
            <textarea
              ref={input}
              rows={1}
              value={draft}
              placeholder="Ask a question, or tell it what to change"
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onKey}
            />
            <button type="button" className="agent-ask-button" onClick={() => submit(false)}>
              Ask {!box.change && enter}
            </button>
            <button
              type="button"
              className={box.change ? "primary" : undefined}
              title={`Have the agent edit the code (${CHANGE_KEY})`}
              onClick={() => submit(true)}
            >
              Change {box.change && enter}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
