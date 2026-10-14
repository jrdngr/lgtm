import { type RefObject, useRef, useState } from "react";
import type { CommentDto, Message } from "../bindings";
import { Inline, Markdown } from "../bots/Markdown";
import { TakePill } from "../threads/ThreadBits";
import { Busy } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { AskBox } from "./AskBox";
import { ProposalSection } from "./ProposalSection";
import type { Comments } from "./useComments";
import { wordDiff } from "./wordDiff";

interface Props {
  comment: CommentDto;
  c: Comments;
  agentsOff: boolean;
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
export function ThreadFlow({ comment, c, agentsOff, editor, askRef }: Props) {
  const draftRef = useRef<HTMLDivElement>(null);
  const [changes, setChanges] = useState(false);
  const [flash, setFlash] = useState(false);
  const id = comment.id;
  const take = comment.thread?.take;
  const body = comment.body;
  const running = c.running.get(id);
  const final = comment.thread?.stage === "ready";
  const made = versions(comment.messages);
  const version = made.size;
  const last = [...made.keys()].at(-1);
  const before = last === undefined ? undefined : comment.messages[last].before;
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
      {!started && !agentsOff && (
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
              <span className="spacer" />
              {version > 1 && before && !final && (
                <button type="button" className="flow-link" onClick={() => setChanges(!changes)}>
                  {changes ? "Hide changes" : "Show changes"}
                </button>
              )}
            </h4>
            {body.trim() ? (
              <div
                ref={draftRef}
                className={`flow-draft-text${final ? " final" : ""}${flash ? " fresh" : ""}`}
              >
                {changes && before ? (
                  <Changes before={before} after={body} />
                ) : (
                  <Markdown>{body}</Markdown>
                )}
              </div>
            ) : (
              <div className="flow-none">No reply</div>
            )}
            <div className="flow-buttons">
              {final ? (
                <>
                  <span className="pill green">✓ Final</span>
                  <span className="faint">Sends with Send to GitHub in the PR sidebar</span>
                  <span className="spacer" />
                  <button type="button" onClick={() => c.edit(comment)}>
                    Edit <kbd>e</kbd>
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

function Changes({ before, after }: { before: string; after: string }) {
  return (
    <p className="flow-changes">
      {wordDiff(before, after).map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
        <span key={i} className={p.kind === "same" ? undefined : `flow-${p.kind}`}>
          {p.text}
        </span>
      ))}
    </p>
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

/** The thread's conversation with the agent. A Draft reply shows only as the agent's answer. */
function Chat({ messages, made, running, reviewing, onShow }: ChatProps) {
  return (
    <div className="flow-chat">
      {messages.map((m, i) => {
        if (m.author === "user") {
          if (m.compose) return null;
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
            <div key={i} className="flow-msg you">
              <span className="flow-who">You</span>
              <span>{m.text || (m.change ? "Make the change this comment asks for." : "")}</span>
            </div>
          );
        }
        const v = made.get(i);
        const changed = !!messages[i - 1]?.change;
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
          <div key={i} className="flow-msg agent">
            <span className="flow-who">{changed ? "Agent · changed the code" : "Agent"}</span>
            <span>{m.text}</span>
            {v !== undefined && (
              <button type="button" className="flow-up" onClick={onShow}>
                ↑ Reply v{v}
              </button>
            )}
          </div>
        );
      })}
      {running && !running.compose && (
        <div className="flow-msg you">
          <span className="flow-who">You</span>
          <span>
            {running.ask || (running.change ? "Make the change this comment asks for." : "")}
          </span>
        </div>
      )}
      {running && (
        <div className="flow-msg agent">
          <span className="flow-who">Agent</span>
          <Busy className={`flow-thinking${running.compose ? " compose" : ""}`}>
            {running.compose
              ? reviewing
                ? "Reviewing reply…"
                : "Drafting a reply…"
              : running.change
                ? "Changing the code…"
                : "Thinking…"}
          </Busy>
        </div>
      )}
    </div>
  );
}
