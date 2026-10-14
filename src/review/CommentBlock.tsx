import { useEffect, useRef, useState } from "react";
import type { CommentDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { GhThread, lastPost } from "../github/GhThread";
import { Avatar } from "../github/Post";
import { viewerLogin } from "../github/people";
import { TakePill } from "../threads/ThreadBits";
import { useAgentsOff } from "../tools/agents";
import { openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { ago, fullDate } from "../ui/time";
import { isMac } from "../workbench/keys";
import { AskBox, SAVE_KEY } from "./AskBox";
import { cardItems, moreItems } from "./cardMenu";
import { NEW_COMMENT } from "./model";
import { StageStrip } from "./StageStrip";
import { ThreadBlock } from "./ThreadBlock";
import type { Comments } from "./useComments";
import { Workshop } from "./Workshop";
import "./comments.css";

export { NOT_AT_WORKING_COPY } from "./AskBox";

interface Props {
  id: number;
  comment?: CommentDto;
  c: Comments;
}

/** A comment under its code: a note of yours, or a GitHub thread or draft of one, with an
 * editor for its text and a conversation with an agent about it. Done ones fold to a line. */
export function CommentBlock({ id, comment, c }: Props) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const askRef = useRef<HTMLTextAreaElement>(null);
  const agentsOff = useAgentsOff();
  const [expanded, setExpanded] = useState(false);
  const [snippet, setSnippet] = useState(false);
  const editing = c.edits.get(id);
  const gh = comment?.github;
  const thread = comment?.thread;
  const viewer = viewerLogin(c.comments);
  const asking = c.asking(id);
  const isNew = id === NEW_COMMENT;
  const github = !!gh || !!thread || (isNew && !!c.draft?.github);
  const done = gh?.resolved || thread?.stage === "done" || (!thread && comment?.done);

  useEffect(() => {
    if (c.focus?.id !== id) return;
    const el = (c.focus.field === "body" ? bodyRef : askRef).current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
    c.focused();
  });

  const card = comment && { comment, c, agentsOff };
  /** Text fields keep their own menu. */
  const onContextMenu = (e: React.MouseEvent) => {
    if (!card || (e.target instanceof Element && e.target.closest("textarea, input"))) return;
    openMenu(e, cardItems(card));
  };

  if (comment && done && !expanded && editing === undefined && !asking) {
    const summary = gh ? lastPost(gh, viewer) : comment.body.split("\n")[0];
    return (
      <div
        className={`comment-block ${github ? "gh" : "mine"} resolved`}
        onContextMenu={onContextMenu}
      >
        <button
          type="button"
          className="card-bar collapsed"
          aria-expanded={false}
          onClick={() => setExpanded(true)}
          title={github ? "Show the thread" : "Show the note"}
        >
          <span className="chevron">▸</span>
          <span className={`pill${gh?.resolved ? " green" : ""}`}>
            {gh?.resolved ? "✓ Resolved" : "✓ Done"}
          </span>
          <span className="card-summary">{thread?.summary || summary}</span>
          <span className="spacer" />
          {gh && <span className="card-count">{gh.posts.length}</span>}
        </button>
      </div>
    );
  }

  if (comment && gh && thread) {
    return (
      <ThreadBlock
        comment={comment}
        c={c}
        agentsOff={agentsOff}
        done={!!done}
        onFold={() => setExpanded(false)}
        onContextMenu={onContextMenu}
        bodyRef={bodyRef}
        askRef={askRef}
      />
    );
  }

  function onBodyKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) c.save(id);
    else if (e.key === "Escape") c.cancel(id);
    else return;
    e.preventDefault();
  }

  const body = editing ?? comment?.body ?? "";
  const changed = comment && !comment.rows;
  const when = comment && (comment.updatedAt ?? comment.createdAt);
  const source = isNew
    ? github
      ? "New draft for GitHub"
      : "New note"
    : gh
      ? thread?.summary || "GitHub"
      : thread
        ? "Draft for GitHub"
        : "Note";
  const canFinal = !!gh && !!body.trim() && thread?.stage !== "ready";
  const finalButton = (
    <button
      type="button"
      className="primary"
      onClick={() => c.final(id)}
      title="Make this the reply to send. It goes out with Send to GitHub in the PR tab."
    >
      Final
    </button>
  );

  return (
    <div
      className={`comment-block ${github ? "gh" : "mine"}${done ? " resolved" : ""}`}
      onContextMenu={onContextMenu}
    >
      <div className="card-bar">
        {done && (
          <button
            type="button"
            className="chevron quiet"
            aria-expanded={true}
            onClick={() => setExpanded(false)}
            title="Fold"
          >
            ▾
          </button>
        )}
        <span className="card-source" title={comment ? `Review: ${comment.review}` : undefined}>
          {source}
        </span>
        {!gh && when ? (
          <>
            <span className="faint">·</span>
            <span className="faint" title={fullDate(when)}>
              {ago(when)}
            </span>
          </>
        ) : null}
        {thread && <TakePill take={thread.take} />}
        {thread && <StageStrip stage={thread.stage} onPick={(st) => c.setStage(id, st)} />}
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
          {comment && !thread && (
            <IconButton
              icon="check"
              label={comment.done ? "Open again" : "Done"}
              aria-pressed={comment.done}
              onClick={() => c.setDone(comment.id, !comment.done)}
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
          {card && <Menu items={moreItems(card)} />}
        </span>
      </div>
      {changed && snippet && <pre className="comment-snippet">{comment.snippet}</pre>}
      {gh && <GhThread link={gh} viewer={viewer} />}
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
            placeholder={
              gh ? "Write a reply" : github ? "Write a comment for GitHub" : "Write a note"
            }
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
              className={canFinal ? undefined : "primary"}
              onClick={() => c.save(id)}
            >
              Save <kbd>{SAVE_KEY}</kbd>
            </button>
            {canFinal && finalButton}
          </div>
        </div>
      ) : (
        gh &&
        comment && (
          <div className="card-foot">
            <button type="button" className="reply-field" onClick={() => c.edit(comment)}>
              {comment.body ? "Edit reply…" : "Reply…"}
            </button>
            {canFinal && finalButton}
          </div>
        )
      )}
      {asking && (
        <div className="card-agent">
          <Workshop
            messages={comment?.messages ?? []}
            running={c.running.get(id)}
            onUse={(t) => c.use(id, t)}
          />
          {!agentsOff && <AskBox id={id} body={body} c={c} inputRef={askRef} />}
        </div>
      )}
    </div>
  );
}
