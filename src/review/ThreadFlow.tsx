import type { CommentDto } from "../bindings";
import { Inline, Markdown } from "../bots/Markdown";
import { TakePill } from "../threads/ThreadBits";
import { Busy, Spinner } from "../ui/Busy";
import { PipeButton } from "../ui/PipeButton";
import { ThreadChat } from "./ThreadChat";
import type { Comments } from "./useComments";

interface Props {
  comment: CommentDto;
  c: Comments;
  /** Whether its chat is synced with a terminal, and whether the agent there is working. */
  chat: { synced: boolean; working: boolean };
  /** A triage run covers this thread, and will set its take, reply and stage. */
  triaging: boolean;
  /** The reply's editor, shown in place of the reply while it's edited. */
  editor?: React.ReactNode;
}

/** A GitHub thread's reply workflow under its posts: the take, the reply, and its chat. */
export function ThreadFlow({ comment, c, chat, triaging, editor }: Props) {
  const id = comment.id;
  const take = comment.thread?.take;
  const body = comment.body;
  const final = comment.thread?.stage === "ready";

  return (
    <div className="flow">
      {triaging && (
        <div className="flow-triaging">
          <Spinner />
          Triaging
        </div>
      )}
      {take && (
        <section className="flow-sec">
          <h4 className="flow-h">Take</h4>
          <div className="flow-take">
            <p>
              <TakePill take={take} />
            </p>
            <div className="flow-why">
              <Markdown>{take.why}</Markdown>
            </div>
            {take.question && (
              <p className="flow-question">
                <Inline>{take.question}</Inline>
              </p>
            )}
          </div>
        </section>
      )}
      {editor ?? (
        <section className="flow-sec">
          <h4 className="flow-h">Reply {final && <span className="flow-ver"> · final</span>}</h4>
          {body.trim() ? (
            <div className={`flow-draft-text${final ? " final" : ""}`}>
              <Markdown>{body}</Markdown>
            </div>
          ) : (
            <div className="flow-none">No reply</div>
          )}
          <div className="flow-buttons">
            {final ? (
              <>
                <span className="pill green">✓ Final</span>
                <span className="spacer" />
                <button type="button" onClick={() => c.edit(comment)}>
                  Edit <kbd>e</kbd>
                </button>
                <button
                  type="button"
                  className="flow-final"
                  disabled={c.sending.has(id)}
                  onClick={() => c.sendOne(id)}
                >
                  {c.sending.has(id) ? <Busy>Sending…</Busy> : "Send to GitHub"}
                </button>
              </>
            ) : (
              <>
                {body.trim() && (
                  <button type="button" className="flow-final" onClick={() => c.final(id)}>
                    Final <kbd>y</kbd>
                  </button>
                )}
                <button type="button" onClick={() => c.edit(comment)}>
                  {body.trim() ? "Edit" : "Write reply"} <kbd>e</kbd>
                </button>
                {body.trim() && (
                  <PipeButton onClick={() => c.reviewReply(comment)}>Review</PipeButton>
                )}
                {!take && comment.thread?.stage === "new" && (
                  <PipeButton onClick={() => c.triage(comment)}>Triage</PipeButton>
                )}
              </>
            )}
          </div>
        </section>
      )}
      <ThreadChat comment={comment} c={c} {...chat} />
    </div>
  );
}
