import type { CommentDto } from "../bindings";
import { Conversation } from "./Conversation";
import { setChatOpen, useChatOpen } from "./linkedAgent";
import "./chat.css";

/** What I and the linked agent said while this was the thread I was on, folded until opened. */
export function ThreadChat({ comment }: { comment: CommentDto }) {
  const open = useChatOpen(comment.id);
  const { messages } = comment;
  if (!messages.length) return null;
  return (
    <section className="flow-chat">
      <button
        type="button"
        className="chat-toggle"
        aria-expanded={open}
        onClick={() => setChatOpen(comment.id, !open)}
      >
        <span className="chevron">{open ? "▾" : "▸"}</span>
        Chat
        <span className="chat-count">{messages.length}</span>
      </button>
      {open && <Conversation messages={messages} />}
    </section>
  );
}
