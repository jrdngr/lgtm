import { type RefObject, useState } from "react";
import type { CommentDto } from "../bindings";
import { GhThread } from "../github/GhThread";
import { viewerLogin } from "../github/people";
import { Icon, IconButton } from "../ui/icons";
import { openUrl } from "../ui/pathMenu";
import { isMac } from "../workbench/keys";
import { SAVE_KEY } from "./AskBox";
import { StageStrip } from "./StageStrip";
import { ThreadFlow } from "./ThreadFlow";
import type { Comments } from "./useComments";

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

/** A GitHub thread under its code: who started it and its stage, its posts, then the reply
 * workflow. `a` drafts or asks, `e` edits the reply, `y` makes it final. */
export function ThreadBlock({ comment, c, agentsOff, done, onFold, ...p }: Props) {
  const [snippet, setSnippet] = useState(false);
  const gh = comment.github;
  const thread = comment.thread;
  if (!gh || !thread) return null;
  const id = comment.id;
  const editing = c.edits.get(id);
  const viewer = viewerLogin(c.comments);
  const changed = !comment.rows;
  const canFinal = !!editing?.trim() && thread.stage !== "ready";

  function onKey(e: React.KeyboardEvent) {
    if (typing(e.target) || e.metaKey || e.ctrlKey || e.altKey) return;
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
      className={`comment-block thread-block${done ? " resolved" : ""}`}
      onContextMenu={p.onContextMenu}
      onKeyDown={onKey}
    >
      <div className="card-bar">
        {done && (
          <button
            type="button"
            className="chevron quiet"
            aria-expanded={true}
            onClick={onFold}
            title="Fold"
          >
            ▾
          </button>
        )}
        <span className="card-source">
          <b>GitHub</b> · {gh.posts[0]?.author ?? "PR"}
        </span>
        <StageStrip stage={thread.stage} onPick={(s) => c.setStage(id, s)} />
        {gh.resolved && <span className="pill green">✓ Resolved</span>}
        {gh.outdated && <span className="pill">Outdated</span>}
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
        <IconButton
          icon="retry"
          label="Reset: clear the take, reply and agent chat"
          disabled={c.running.has(id)}
          onClick={() => c.reset(id)}
        />
        <IconButton icon="external" label="Open on GitHub" onClick={() => openUrl(gh.url)} />
        <button type="button" className="card-send" onClick={() => c.send(comment)}>
          <Icon name="send" size={13} />
          Send to terminal
        </button>
      </div>
      {changed && snippet && <pre className="comment-snippet">{comment.snippet}</pre>}
      <GhThread link={gh} viewer={viewer} />
      <ThreadFlow
        comment={comment}
        c={c}
        agentsOff={agentsOff}
        editor={editor || undefined}
        askRef={p.askRef}
      />
    </div>
  );
}
