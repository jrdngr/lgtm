import { type RefObject, useRef, useState } from "react";
import type { CommentDto, Message } from "../bindings";
import { Inline, Markdown } from "../bots/Markdown";
import { TakePill } from "../threads/ThreadBits";
import { Busy, Spinner } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { AskBox } from "./AskBox";
import { ProposalSection } from "./ProposalSection";
import { SuggestedReply } from "./SuggestedReply";
import type { Comments } from "./useComments";

interface Props {
  comment: CommentDto;
  c: Comments;
  agentsOff: boolean;
  /** A triage run covers this thread, and will set its take, reply and stage. */
  triaging: boolean;
  /** The reply's editor, shown in place of the reply while it's edited. */
  editor?: React.ReactNode;
  askRef: RefObject<HTMLTextAreaElement | null>;
}

/** Agent turns that replaced the reply, each with the reply's version it made. */
function versions(messages: Message[]) {
  const made = new Map<number, number>();
  for (const [i, m] of messages.entries()) {
    if (m.author === "agent" && m.before != null) made.set(i, made.size + 1);
  }
  return made;
}

/** A GitHub thread's reply workflow under its posts: Draft reply to start, then the agent's
 * take, the reply draft, and the conversation that refines it. */
export function ThreadFlow({ comment, c, agentsOff, triaging, editor, askRef }: Props) {
  const draftRef = useRef<HTMLDivElement>(null);
  const [flash, setFlash] = useState(false);
  const id = comment.id;
  const take = comment.thread?.take;
  const body = comment.body;
  const running = c.running.get(id);
  const final = comment.thread?.stage === "ready";
  const made = versions(comment.messages);
  const version = made.size;
  const suggestion = comment.thread?.suggestion;
  const started = !!take || !!body.trim() || comment.messages.length > 0 || !!running;

  function showDraft() {
    draftRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    setFlash(true);
    setTimeout(() => setFlash(false), 1200);
  }

  const versionText = final
    ? version
      ? ` · v${version} · final`
      : " · final"
    : version
      ? ` · v${version}${version > 1 ? " · updated by the agent" : ""}`
      : "";

  return (
    <div className="flow">
      {triaging && (
        <div className="flow-triaging">
          <Spinner />
          Triaging
        </div>
      )}
      {!started && !agentsOff && !triaging && (
        <div className="flow-start">
          <button type="button" className="flow-draft" onClick={() => c.compose(id)}>
            <Icon name="sparkle" size={13} />
            Draft reply <kbd>a</kbd>
          </button>
        </div>
      )}
      {take && (
        <section className="flow-sec">
          <h4 className="flow-h">Take</h4>
          <div className="flow-take">
            <p>
              <TakePill take={take} />
            </p>
            <p>
              <Inline>{take.why}</Inline>
            </p>
            {take.question && (
              <p className="flow-question">
                <Inline>{take.question}</Inline>
              </p>
            )}
          </div>
        </section>
      )}
      {editor ??
        ((body.trim() || take) && (
          <section className="flow-sec">
            <h4 className="flow-h">
              Reply <span className="flow-ver">{versionText}</span>
            </h4>
            {body.trim() ? (
              <div
                ref={draftRef}
                className={`flow-draft-text${final ? " final" : ""}${flash ? " fresh" : ""}`}
              >
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
                  {!agentsOff && (
                    <button
                      type="button"
                      className="flow-draft"
                      disabled={!!running}
                      onClick={() => c.compose(id)}
                    >
                      <Icon name="sparkle" size={13} />
                      Review
                    </button>
                  )}
                </>
              )}
            </div>
          </section>
        ))}
      {!editor && !final && suggestion && (
        <SuggestedReply id={id} body={body} suggestion={suggestion} c={c} />
      )}
      {!final && (comment.messages.length > 0 || running) && (
        <section className="flow-sec">
          {(take || body.trim()) && <h4 className="flow-h">Agent</h4>}
          <Chat
            messages={comment.messages}
            made={made}
            running={running}
            reviewing={!!body.trim()}
            onShow={showDraft}
          />
        </section>
      )}
      {!final && !agentsOff && (
        <AskBox
          id={id}
          body={body}
          c={c}
          inputRef={askRef}
          off={triaging}
          placeholder={
            started
              ? "Ask a question, or tell it what to change"
              : "Or ask the agent about this comment first"
          }
        />
      )}
      <ProposalSection comment={comment} c={c} />
    </div>
  );
}

interface ChatProps {
  messages: Message[];
  made: Map<number, number>;
  running: ReturnType<Comments["running"]["get"]>;
  /** A compose run reviews the reply there is rather than drafting a first one. */
  reviewing: boolean;
  onShow(): void;
}

/** One turn of the conversation: who spoke, in a column of its own, and what they said. */
function Turn({ who, children }: { who: "you" | "agent"; children: React.ReactNode }) {
  return (
    <div className={`flow-turn ${who}`}>
      <span className="flow-who">{who === "you" ? "You" : "Agent"}</span>
      <div className="flow-said">{children}</div>
    </div>
  );
}

/** The thread's conversation with the agent. A Draft reply shows only as the agent's answer. */
function Chat({ messages, made, running, reviewing, onShow }: ChatProps) {
  const asked = (text: string, change?: boolean) =>
    text || (change ? "Make the change this comment asks for." : "");
  return (
    <div className="flow-chat">
      {messages.map((m, i) => {
        if (m.author === "user") {
          if (m.compose) return null;
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
            <Turn key={i} who="you">
              <p>{asked(m.text, m.change)}</p>
            </Turn>
          );
        }
        const changed = !!messages[i - 1]?.change;
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
          <Turn key={i} who="agent">
            <Markdown>{m.text}</Markdown>
            {made.has(i) && (
              <button type="button" className="flow-event reply" onClick={onShow}>
                Reply updated
              </button>
            )}
            {changed && <span className="flow-event code">Code changed</span>}
          </Turn>
        );
      })}
      {running && !running.compose && (
        <Turn who="you">
          <p>{asked(running.ask, running.change)}</p>
        </Turn>
      )}
      {running && (
        <Turn who="agent">
          <Busy className={`flow-thinking${running.compose ? " compose" : ""}`}>
            {running.compose
              ? reviewing
                ? "Reviewing reply…"
                : "Drafting a reply…"
              : running.change
                ? "Changing the code…"
                : "Thinking…"}
          </Busy>
        </Turn>
      )}
    </div>
  );
}
