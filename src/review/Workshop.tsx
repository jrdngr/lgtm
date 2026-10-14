import type { Message } from "../bindings";
import { Busy } from "../ui/Busy";
import { replyParts } from "./comments";
import type { Running } from "./useComments";

interface TextProps {
  text: string;
  /** The comment's saved text, to mark the suggestion it already is. */
  body?: string;
  onUse?(text: string): void;
}

function AgentText({ text, body, onUse }: TextProps) {
  return (
    <>
      {replyParts(text).map((p, i) =>
        p.kind === "text" ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
          <div key={i} className="msg-text">
            {p.text}
          </div>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
          <div key={i} className="suggestion">
            <div className="msg-text">{p.text}</div>
            {onUse &&
              (body?.trim() === p.text.trim() ? (
                <span className="pill green">✓ Used</span>
              ) : (
                <button type="button" onClick={() => onUse(p.text)}>
                  Use this
                </button>
              ))}
          </div>
        ),
      )}
    </>
  );
}

function who(m: Message) {
  if (m.author === "agent") return m.updated ? "Agent · updated take and reply" : "Agent";
  return m.change ? "You · change" : "You";
}

function Msg({ m, body, onUse }: { m: Message; body?: string; onUse(text: string): void }) {
  const text = m.text || (m.change ? "Make the change this comment asks for." : "");
  return (
    <div className={`msg msg-${m.author}`}>
      <span className="msg-who">{who(m)}</span>
      {m.author === "agent" ? (
        <AgentText text={text} body={body} onUse={onUse} />
      ) : (
        <div className="msg-text">{text}</div>
      )}
    </div>
  );
}

interface Props {
  messages: Message[];
  running?: Running;
  body?: string;
  onUse(text: string): void;
}

/** A comment's conversation with the agent, including the exchange in flight. */
export function Workshop({ messages, running, body, onUse }: Props) {
  if (!messages.length && !running) return null;
  return (
    <div className="workshop">
      {messages.map((m, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
        <Msg key={i} m={m} body={body} onUse={onUse} />
      ))}
      {running && (
        <>
          <Msg m={{ author: "user", text: running.ask, change: running.change }} onUse={onUse} />
          <div className="msg msg-agent">
            <span className="msg-who">Agent</span>
            {running.reply ? (
              <AgentText text={running.reply} />
            ) : (
              <Busy>{running.change ? "Changing the code…" : "Thinking…"}</Busy>
            )}
          </div>
        </>
      )}
    </div>
  );
}
