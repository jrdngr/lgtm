import { useEffect, useRef } from "react";
import type { CommentDto, Message } from "../bindings";
import { isMac } from "../workbench/keys";
import { replyParts } from "./comments";
import { NEW_COMMENT } from "./model";
import type { Comments } from "./useComments";
import "./comments.css";

export const NOT_AT_WORKING_COPY =
  "The agent edits the working copy (@), which this review doesn't include. Review a revset that includes @ to make changes.";

const SAVE_KEY = isMac ? "⌘↵" : "Ctrl+↵";

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
  id: number;
  comment?: CommentDto;
  c: Comments;
}

/** A comment under its code: the body, or an editor for it, and a conversation with an agent
 * about it. */
export function CommentBlock({ id, comment, c }: Props) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const askRef = useRef<HTMLTextAreaElement>(null);
  const editing = c.edits.get(id);
  const running = c.running.get(id);
  const isNew = id === NEW_COMMENT;

  useEffect(() => {
    if (c.focus?.id !== id) return;
    const el = (c.focus.field === "body" ? bodyRef : askRef).current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
    c.focused();
  });

  function onBodyKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) c.save(id);
    else if (e.key === "Escape") c.cancel(id);
    else return;
    e.preventDefault();
  }

  function onAskKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Enter" && mod) {
      if (c.canChange) c.ask(id, true);
    } else if (e.key === "Enter" && !e.shiftKey) c.ask(id, false);
    else if (e.key === "Escape") c.done();
    else return;
    e.preventDefault();
  }

  const outdated = comment && !comment.rows;
  const asked = !!c.asks.get(id)?.trim();
  const body = editing ?? comment?.body ?? "";
  const use = (text: string) => c.use(id, text);
  return (
    <div className="comment-block">
      <div className="comment-head">
        <span className="comment-title">{isNew ? "New comment" : "Comment"}</span>
        {comment && (
          <span
            className="comment-meta"
            title={
              comment.updatedAt === null ? undefined : new Date(comment.updatedAt).toLocaleString()
            }
          >
            {comment.review}
          </span>
        )}
        {outdated && <span className="comment-outdated">code changed</span>}
        <span className="spacer" />
        {comment && editing === undefined && (
          <>
            <button type="button" onClick={() => c.edit(comment)}>
              Edit
            </button>
            <button
              type="button"
              onClick={() => c.send(comment)}
              title="Paste into the agent's prompt"
            >
              Send to agent
            </button>
            <button type="button" onClick={() => c.remove(comment.id)}>
              Delete
            </button>
          </>
        )}
      </div>
      {outdated && <pre className="comment-snippet">{comment.snippet}</pre>}
      {editing === undefined ? (
        <div className="comment-body">{comment?.body || <em>No text yet</em>}</div>
      ) : (
        <>
          <textarea
            ref={bodyRef}
            className="comment-input"
            value={editing}
            rows={Math.min(12, Math.max(3, editing.split("\n").length))}
            placeholder="Write a comment"
            onChange={(e) => c.setBody(id, e.target.value)}
            onKeyDown={onBodyKey}
          />
          <div className="comment-actions">
            <button type="button" className="primary" onClick={() => c.save(id)}>
              Save <kbd>{SAVE_KEY}</kbd>
            </button>
            <button type="button" onClick={() => c.cancel(id)}>
              Cancel <kbd>Esc</kbd>
            </button>
          </div>
        </>
      )}
      {(!!comment?.messages.length || running) && (
        <div className="workshop">
          {comment?.messages.map((m, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only
            <Msg key={i} m={m} onUse={use} />
          ))}
          {running && (
            <>
              <Msg m={{ author: "user", text: running.ask, change: running.change }} onUse={use} />
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
      )}
      <div className="ask">
        <textarea
          ref={askRef}
          className="ask-input"
          rows={1}
          value={c.asks.get(id) ?? ""}
          placeholder={`Ask the agent (↵), or tell it what to change (${SAVE_KEY})`}
          onChange={(e) => c.setAsk(id, e.target.value)}
          onKeyDown={onAskKey}
        />
        {running ? (
          <button type="button" onClick={() => c.stop(id)}>
            Stop
          </button>
        ) : (
          <>
            <button type="button" onClick={() => c.ask(id, false)} disabled={!asked}>
              Ask
            </button>
            <button
              type="button"
              onClick={() => c.ask(id, true)}
              disabled={!c.canChange || (!asked && !body.trim())}
              title={
                c.canChange
                  ? "Have the agent edit the code. With nothing typed, it does what the comment says."
                  : NOT_AT_WORKING_COPY
              }
            >
              Change
            </button>
          </>
        )}
      </div>
    </div>
  );
}
