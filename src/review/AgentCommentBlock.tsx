import { useEffect, useRef } from "react";
import { isMac, SAVE_KEY } from "../core/keys";
import { Avatar } from "../github/Post";
import { Markdown } from "../reports/Markdown";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { NEW_AGENT_COMMENT } from "./agentComments";
import type { AgentComments } from "./useAgentComments";
import "./comments.css";

interface Props {
  id: number;
  a: AgentComments;
  /** Who's writing, for the avatar. */
  viewer?: string;
}

/** A comment for the agent under its code: C# and its text, or an editor for it. Any folds to a line;
 * resolved ones start folded. */
export function AgentCommentBlock({ id, a, viewer }: Props) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const comment = a.find(id);
  const editing = a.edits.get(id);
  const number = comment?.number ?? a.next;

  useEffect(() => {
    if (a.focus !== id) return;
    const el = bodyRef.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
    a.focused();
  });

  if (!comment && id !== NEW_AGENT_COMMENT) return null;

  if (comment && editing === undefined && comment.done !== a.toggled.has(id)) {
    return (
      <div className="comment-block folded resolved">
        <button
          type="button"
          className="card-bar collapsed"
          aria-expanded={false}
          onClick={() => a.toggle(id)}
        >
          <span className="chevron">▸</span>
          {comment.done && <span className="pill green">✓ Resolved</span>}
          <span className="card-source">C{number}</span>
          <span className="card-summary">{comment.body.split("\n")[0]}</span>
          <span className="spacer" />
        </button>
      </div>
    );
  }

  function onBodyKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) a.save(id);
    else if (e.key === "Escape") a.cancel(id);
    else return;
    e.preventDefault();
  }

  return (
    <div className={`comment-block${comment?.done ? " resolved" : ""}`}>
      <div className="card-bar">
        {comment && (
          <button
            type="button"
            className="chevron quiet"
            aria-expanded={true}
            aria-label="Fold"
            onClick={() => a.toggle(id)}
          >
            ▾
          </button>
        )}
        <span className="card-source">C{number}</span>
        {comment?.done && <span className="pill green">✓ Resolved</span>}
        <span className="spacer" />
        {comment && editing === undefined && (
          <span className="card-actions reveal">
            <button
              type="button"
              className="quiet"
              onClick={() => a.setDone(comment, !comment.done)}
            >
              {comment.done ? "Reopen" : "✓ Resolve"}
            </button>
            <IconButton icon="edit" label="Edit" onClick={() => a.edit(comment)} />
            <IconButton
              icon="terminal"
              className="pipe-icon"
              label="Send to Terminal"
              onClick={() => a.send(comment)}
            />
            <Menu items={[{ label: "Delete", danger: true, onSelect: () => a.remove(comment) }]} />
          </span>
        )}
      </div>
      {editing === undefined && comment ? (
        <div className="post">
          <Avatar name={viewer ?? "You"} />
          <div className="post-body">
            <Markdown>{comment.body}</Markdown>
          </div>
        </div>
      ) : (
        <div className="card-foot editing">
          <textarea
            ref={bodyRef}
            className="comment-input"
            value={editing ?? ""}
            rows={Math.min(12, Math.max(3, (editing ?? "").split("\n").length))}
            onChange={(e) => a.setBody(id, e.target.value)}
            onKeyDown={onBodyKey}
          />
          <div className="card-buttons">
            <span className="spacer" />
            <button type="button" className="quiet" onClick={() => a.cancel(id)}>
              Cancel <kbd>Esc</kbd>
            </button>
            <button type="button" className="primary" onClick={() => a.save(id)}>
              Save <kbd>{SAVE_KEY}</kbd>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
