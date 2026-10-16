import type { Message } from "../bindings";
import { Markdown } from "../bots/Markdown";

/** A comment's conversation: replies terminal agents gave on it. */
export function Conversation({ messages }: { messages: Message[] }) {
  if (!messages.length) return null;
  return (
    <div className="conversation">
      {messages.map((m, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
        <div key={i} className={`msg msg-${m.author}`}>
          <span className="msg-who">{m.author === "agent" ? "Agent" : "You"}</span>
          {m.author === "agent" ? (
            <div className="msg-md">
              <Markdown>{m.text}</Markdown>
            </div>
          ) : (
            <div className="msg-text">{m.text}</div>
          )}
        </div>
      ))}
    </div>
  );
}
