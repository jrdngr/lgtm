import { useEffect, useRef, useState } from "react";
import type { CommentDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { GhThread, lastPost } from "../github/GhThread";
import { Avatar, Post } from "../github/Post";
import { viewerLogin } from "../github/people";
import { useAgentsOff } from "../tools/agents";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { ago, fullDate } from "../ui/time";
import { isMac } from "../workbench/keys";
import { AskBox, SAVE_KEY } from "./AskBox";
import { NEW_COMMENT } from "./model";
import type { Comments } from "./useComments";
import { Workshop } from "./Workshop";
import "./comments.css";

export { NOT_AT_WORKING_COPY } from "./AskBox";

interface Props {
  id: number;
  comment?: CommentDto;
  c: Comments;
}

/** A comment under its code: a GitHub thread or your own comment, an editor for its text, and
 * a conversation with an agent about it. */
export function CommentBlock({ id, comment, c }: Props) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const askRef = useRef<HTMLTextAreaElement>(null);
  const agentsOff = useAgentsOff();
  const [expanded, setExpanded] = useState(false);
  const [snippet, setSnippet] = useState(false);
  const editing = c.edits.get(id);
  const gh = comment?.github;
  const viewer = viewerLogin(c.comments);
  const asking = c.asking(id);

  useEffect(() => {
    if (c.focus?.id !== id) return;
    const el = (c.focus.field === "body" ? bodyRef : askRef).current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
    c.focused();
  });

  if (gh?.resolved && !expanded && editing === undefined && !asking) {
    return (
      <div className="comment-block gh resolved">
        <button
          type="button"
          className="card-bar collapsed"
          aria-expanded={false}
          onClick={() => setExpanded(true)}
          title="Show the thread"
        >
          <span className="chevron">▸</span>
          <span className="pill green">✓ Resolved</span>
          <span className="card-summary">{lastPost(gh, viewer)}</span>
          <span className="spacer" />
          <span className="card-count">{gh.posts.length}</span>
        </button>
      </div>
    );
  }

  function onBodyKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) c.save(id);
    else if (e.key === "Escape") c.cancel(id);
    else return;
    e.preventDefault();
  }

  const isNew = id === NEW_COMMENT;
  const body = editing ?? comment?.body ?? "";
  const changed = comment && !comment.rows;
  const publishing = c.publishing.has(id);
  const canPublish = c.canPublish && !!body.trim() && !publishing;
  const publishTitle = "Add to your pending review on GitHub. Submit it from the Comments view.";
  const when = comment && (comment.updatedAt ?? comment.createdAt);
  const kind = gh ? "gh" : "mine";

  const publishButton = c.canPublish && (
    <button
      type="button"
      className="primary"
      onClick={() => c.publish(id)}
      disabled={!canPublish}
      title={publishTitle}
    >
      {publishing ? "Publishing…" : "Publish"}
    </button>
  );

  return (
    <div className={`comment-block ${kind}${gh?.resolved ? " resolved" : ""}`}>
      <div className="card-bar">
        {gh?.resolved && (
          <button
            type="button"
            className="chevron quiet"
            aria-expanded={true}
            onClick={() => setExpanded(false)}
            title="Fold the thread"
          >
            ▾
          </button>
        )}
        <span className="card-source" title={comment ? `Review: ${comment.review}` : undefined}>
          {isNew ? "New comment" : gh ? "GitHub" : "Your comment"}
        </span>
        {gh && (
          <>
            <span className="faint">·</span>
            <span>
              {gh.posts.length} {gh.posts.length === 1 ? "post" : "posts"}
            </span>
          </>
        )}
        {!gh && when ? (
          <>
            <span className="faint">·</span>
            <span className="faint" title={fullDate(when)}>
              {ago(when)}
            </span>
          </>
        ) : null}
        {gh?.resolved && <span className="pill green">✓ Resolved</span>}
        {gh?.outdated && <span className="pill">Outdated</span>}
        {changed && (
          <button
            type="button"
            className="pill amber"
            aria-expanded={snippet}
            onClick={() => setSnippet(!snippet)}
            title={snippet ? "Hide the code as it was" : "Show the code as it was"}
          >
            Code changed {snippet ? "▾" : "▸"}
          </button>
        )}
        <span className="spacer" />
        <span className={`card-actions${editing === undefined ? " reveal" : ""}`}>
          {comment && editing === undefined && (!gh || comment.body) && (
            <IconButton
              icon="edit"
              label={gh ? "Edit reply" : "Edit"}
              onClick={() => c.edit(comment)}
            />
          )}
          {!agentsOff && (
            <IconButton
              icon="sparkle"
              label="Ask agent (a)"
              aria-pressed={asking}
              onClick={() => (asking ? c.closeAsk(id) : c.openAsk(id))}
            />
          )}
          {comment && (
            <IconButton icon="send" label="Send to terminal" onClick={() => c.send(comment)} />
          )}
          {comment && (
            <Menu
              items={[
                editing === undefined &&
                  !gh &&
                  c.canPublish && {
                    label: "Publish",
                    onSelect: () => c.publish(id),
                    disabled: !canPublish,
                    title: publishTitle,
                  },
                { label: "Delete", onSelect: () => c.remove(comment.id), danger: true },
              ]}
            />
          )}
        </span>
      </div>
      {changed && snippet && <pre className="comment-snippet">{comment.snippet}</pre>}
      {gh && <GhThread link={gh} viewer={viewer} />}
      {editing === undefined && gh && comment?.body && (
        <Post
          author={viewer ?? "You"}
          name="You"
          pills={<span className="pill blue">Not published</span>}
          body={comment.body}
        />
      )}
      {editing === undefined && !gh && (
        <div className="post">
          <Avatar name={viewer ?? "You"} />
          <div className="post-body">
            {comment?.body ? <Markdown>{comment.body}</Markdown> : <em>No text yet</em>}
          </div>
        </div>
      )}
      {editing !== undefined ? (
        <div className="card-foot editing">
          <textarea
            ref={bodyRef}
            className="comment-input"
            value={editing}
            rows={Math.min(12, Math.max(3, editing.split("\n").length))}
            placeholder={gh ? "Write a reply" : "Write a comment"}
            onChange={(e) => c.setBody(id, e.target.value)}
            onKeyDown={onBodyKey}
          />
          <div className="card-buttons">
            <span className="spacer" />
            <button type="button" className="quiet" onClick={() => c.cancel(id)}>
              Cancel <kbd>Esc</kbd>
            </button>
            <button
              type="button"
              className={c.canPublish ? undefined : "primary"}
              onClick={() => c.save(id)}
            >
              Save <kbd>{SAVE_KEY}</kbd>
            </button>
            {publishButton}
          </div>
        </div>
      ) : (
        gh &&
        comment && (
          <div className="card-foot">
            <button type="button" className="reply-field" onClick={() => c.edit(comment)}>
              {comment.body ? "Edit reply…" : "Reply…"}
            </button>
            {comment.body && publishButton}
          </div>
        )
      )}
      {asking && (
        <div className="card-agent">
          <Workshop comment={comment} running={c.running.get(id)} onUse={(t) => c.use(id, t)} />
          {!agentsOff && <AskBox id={id} body={body} c={c} inputRef={askRef} />}
        </div>
      )}
    </div>
  );
}
