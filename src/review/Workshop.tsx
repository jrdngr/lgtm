import type { CommentDto, Message } from "../bindings";
import { replyParts } from "./comments";
import type { Running } from "./useComments";

function AgentText({ text, onUse }: { text: string; onUse?(text: string): void }) {
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
            {onUse && (
              <button type="button" onClick={() => onUse(p.text)}>
                Use this
              </button>
            )}
          </div>
        ),
      )}
    </>
  );
}

function who(m: Message) {
  if (m.author === "agent") return "Agent";
  return m.change ? "You · change" : "You";
}

function Msg({ m, onUse }: { m: Message; onUse(text: string): void }) {
  const text = m.text || (m.change ? "Make the change this comment asks for." : "");
  return (
    <div className={`msg msg-${m.author}`}>
      <span className="msg-who">{who(m)}</span>
      {m.author === "agent" ? (
        <AgentText text={text} onUse={onUse} />
      ) : (
        <div className="msg-text">{text}</div>
      )}
    </div>
  );
}

interface Props {
  comment?: CommentDto;
  running?: Running;
  onUse(text: string): void;
}

/** A comment's conversation with the agent, including the exchange in flight. */
export function Workshop({ comment, running, onUse }: Props) {
  if (!comment?.messages.length && !running) return null;
  return (
    <div className="workshop">
      {comment?.messages.map((m, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
        <Msg key={i} m={m} onUse={onUse} />
      ))}
      {running && (
        <>
          <Msg m={{ author: "user", text: running.ask, change: running.change }} onUse={onUse} />
          <div className="msg msg-agent">
            <span className="msg-who">Agent</span>
            {running.reply ? (
              <AgentText text={running.reply} />
            ) : (
              <em>{running.change ? "Changing the code…" : "Thinking…"}</em>
            )}
          </div>
        </>
      )}
    </div>
  );
}
