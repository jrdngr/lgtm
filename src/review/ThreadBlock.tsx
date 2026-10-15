import { type RefObject, useRef, useState } from "react";
import type { CommentDto } from "../bindings";
import { GhThread } from "../github/GhThread";
import { viewerLogin } from "../github/people";
import { useTriaging } from "../threads/useThreadRun";
import { IconButton } from "../ui/icons";
import { openUrl } from "../ui/pathMenu";
import { isMac } from "../workbench/keys";
import { ApplyDialog } from "./ApplyDialog";
import { SAVE_KEY } from "./AskBox";
import { CombineDialog } from "./CombineDialog";
import { quotesOf } from "./prComments";
import { SnippetDiff } from "./SnippetDiff";
import { StageStrip } from "./StageStrip";
import { ThreadFlow } from "./ThreadFlow";
import type { Comments } from "./useComments";
import { useQuoting } from "./useQuoting";

interface Props {
  comment: CommentDto;
  c: Comments;
  agentsOff: boolean;
  done: boolean;
  onFold(): void;
  onContextMenu(e: React.MouseEvent): void;
  bodyRef: RefObject<HTMLTextAreaElement | null>;
  askRef: RefObject<HTMLTextAreaElement | null>;
}

const typing = (t: EventTarget) =>
  t instanceof HTMLTextAreaElement || t instanceof HTMLInputElement;

/** A GitHub thread under its code, a comment on the PR, or a quote of part of one: who started
 * it and its stage, its posts, then the reply workflow. `a` drafts or asks, `e` edits the
 * reply, `y` makes it final. In a comment on the PR, `q` quotes the selected text; once it has
 * quotes, they carry its replies. */
export function ThreadBlock({ comment, c, agentsOff, done, onFold, ...p }: Props) {
  const [snippet, setSnippet] = useState(false);
  const [combining, setCombining] = useState(false);
  const triaging = useTriaging(c.reviewId, comment.id);
  const blockRef = useRef<HTMLDivElement>(null);
  const postsRef = useRef<HTMLDivElement>(null);
  const quoting = useQuoting(comment, c, postsRef, blockRef);
  const gh = comment.github;
  const thread = comment.thread;
  const quote = thread?.quote;
  if (!thread || (!gh && !quote)) return null;
  const id = comment.id;
  const editing = c.edits.get(id);
  const viewer = viewerLogin(c.comments);
  const changed = !!comment.path && !comment.rows;
  const canFinal = !!editing?.trim() && thread.stage !== "ready";
  const onPr = !!gh && !comment.path;
  const quotes = onPr ? quotesOf(c.onPr, id) : [];
  const split = quotes.length > 0;
  const combinable = quotes.filter((q) => q.body.trim() && q.thread?.stage !== "done");
  const numbers = new Map(quotes.map((q, i) => [q.id, i + 1]));
  const number = quote ? quotesOf(c.onPr, quote.of).findIndex((q) => q.id === id) + 1 : 0;

  function onKey(e: React.KeyboardEvent) {
    if (typing(e.target) || e.metaKey || e.ctrlKey || e.altKey) return;
    if (triaging || split) return;
    if (e.key === "a" && !agentsOff) c.agentKey(id);
    else if (e.key === "e") c.edit(comment);
    else if (e.key === "y" && comment.body.trim() && thread?.stage !== "ready") c.final(id);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function onBodyKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) c.save(id);
    else if (e.key === "Escape") c.cancel(id);
    else return;
    e.preventDefault();
  }

  const editor = editing !== undefined && (
    <section className="flow-sec">
      <h4 className="flow-h">Reply</h4>
      <textarea
        ref={p.bodyRef}
        className="comment-input"
        value={editing}
        rows={Math.min(12, Math.max(3, editing.split("\n").length))}
        placeholder="Write a reply"
        onChange={(e) => c.setBody(id, e.target.value)}
        onKeyDown={onBodyKey}
      />
      <div className="flow-buttons">
        <span className="spacer" />
        <button type="button" className="quiet" onClick={() => c.cancel(id)}>
          Cancel <kbd>Esc</kbd>
        </button>
        <button type="button" onClick={() => c.save(id)}>
          Save <kbd>{SAVE_KEY}</kbd>
        </button>
        {canFinal && (
          <button type="button" className="flow-final" onClick={() => c.final(id)}>
            Final
          </button>
        )}
      </div>
    </section>
  );

  return (
    <div
      ref={blockRef}
      className={`comment-block thread-block${comment.file === -1 ? " on-pr" : ""}${quote ? " quote-block" : ""}${done ? " resolved" : ""}`}
      onContextMenu={p.onContextMenu}
      onKeyDown={onKey}
    >
      <div className="card-bar">
        <button
          type="button"
          className="chevron quiet"
          aria-expanded={true}
          aria-label="Fold"
          onClick={onFold}
        >
          ▾
        </button>
        <span className="card-source">
          <b>GitHub</b> ·{" "}
          {quote ? (
            <>
              <span className="quote-num">{number}</span>quote
            </>
          ) : (
            (gh?.posts[0]?.author ?? "PR")
          )}
          {comment.file === -1 && comment.path && (
            <span className="card-path"> · {comment.path}</span>
          )}
        </span>
        {!split && (
          <StageStrip stage={thread.stage} locked={triaging} onPick={(s) => c.setStage(id, s)} />
        )}
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
        {!split && (
          <IconButton
            icon="retry"
            label="Reset"
            disabled={c.running.has(id) || triaging}
            onClick={() => c.reset(id)}
          />
        )}
        {gh && (
          <IconButton icon="external" label="Open on GitHub" onClick={() => openUrl(gh.url)} />
        )}
        <IconButton icon="terminal" label="Send to terminal" onClick={() => c.send(comment)} />
      </div>
      {changed && snippet && <SnippetDiff snippet={comment.snippet} />}
      {gh && (
        <div ref={postsRef} onMouseUp={onPr ? quoting.onMouseUp : undefined}>
          <GhThread link={gh} viewer={viewer} />
        </div>
      )}
      {quote && <blockquote className="quote-text quote-own">{quote.text}</blockquote>}
      {quoting.quoteButton}
      {c.confirming === id && <ApplyDialog comment={comment} c={c} />}
      {split ? (
        <div className="flow quote-bar">
          <span className="pill purple">
            {quotes.length} {quotes.length === 1 ? "quote" : "quotes"}
          </span>
          <span className="spacer" />
          <button type="button" disabled={combinable.length < 2} onClick={() => setCombining(true)}>
            Combine responses
          </button>
        </div>
      ) : (
        <ThreadFlow
          comment={comment}
          c={c}
          agentsOff={agentsOff}
          triaging={triaging}
          editor={editor || undefined}
          askRef={p.askRef}
        />
      )}
      {combining && (
        <CombineDialog
          quotes={combinable}
          numbers={numbers}
          c={c}
          onClose={() => setCombining(false)}
        />
      )}
    </div>
  );
}
