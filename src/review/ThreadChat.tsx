import type { CommentDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { PipeButton } from "../ui/PipeButton";
import { Conversation } from "./Conversation";
import "./chat.css";
import type { Comments } from "./useComments";

interface Props {
  comment: CommentDto;
  c: Comments;
  synced: boolean;
  working: boolean;
}

/** A thread's chat with the agent in a synced terminal, and the reply it suggests. Shown once
 * it's synced or has something to show. */
export function ThreadChat({ comment, c, synced, working }: Props) {
  const { messages, suggestion } = comment;
  if (!synced && !messages.length && !suggestion) return null;
  return (
    <section className="flow-sec flow-chat">
      <h4 className="flow-h">
        Chat
        {synced ? (
          <>
            <span className="chat-synced">
              <span className={`chat-dot${working ? "" : " still"}`} />
              Synced
            </span>
            <button type="button" className="chat-stop" onClick={c.stopSync}>
              <i />
              Stop
            </button>
          </>
        ) : (
          <PipeButton onClick={() => c.sync(comment)}>Resume</PipeButton>
        )}
      </h4>
      {(messages.length > 0 || working) && (
        <div className="chat-conv">
          <Conversation messages={messages} />
          {working && (
            <div className="chat-work">
              <span className="chat-spin" />
              Working
            </div>
          )}
        </div>
      )}
      {suggestion && (
        <div className="chat-sug">
          <span className="chat-sug-label">Suggested reply</span>
          <div className="msg-md">
            <Markdown>{suggestion}</Markdown>
          </div>
          <div className="flow-buttons">
            <button type="button" onClick={() => c.takeSuggestion(comment.id)}>
              Use
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
