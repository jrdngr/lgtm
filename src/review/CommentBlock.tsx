import { useEffect, useRef } from "react";
import type { CommentDto } from "../bindings";
import { isMac } from "../workbench/keys";
import { NEW_COMMENT } from "./model";
import type { Comments } from "./useComments";
import { Workshop } from "./Workshop";
import "./comments.css";

export const NOT_AT_WORKING_COPY =
  "The agent edits the working copy (@), which this review doesn't include. Review a revset that includes @ to make changes.";

const SAVE_KEY = isMac ? "⌘↵" : "Ctrl+↵";

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
      <Workshop comment={comment} running={running} onUse={use} />
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
