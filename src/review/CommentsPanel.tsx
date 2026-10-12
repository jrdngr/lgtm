import type { CommentDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { useWorkspace } from "../workspaces/context";
import { commentText, rowLabel } from "./comments";
import { type CurrentReview, useCurrentReview } from "./current";
import "./comments.css";

/** A comment's text, or for a GitHub thread its last post, with who wrote it. */
function preview(c: CommentDto) {
  if (c.body || !c.github) return c.body || "No text yet";
  const last = c.github.posts[c.github.posts.length - 1];
  return last ? `${last.author}: ${last.body}` : "GitHub thread";
}

/** The current review's comments in file order, under the PR in a PR workspace. Click one to
 * jump to it. */
export function CommentsPanel() {
  const review = useCurrentReview(useWorkspace());
  return (
    <div className="comment-list">
      <PrSection review={review} />
      {review ? <Comments review={review} /> : <div className="placeholder">No review open</div>}
    </div>
  );
}

function Comments({ review }: { review: CurrentReview }) {
  const { comments, files } = review;
  if (comments.length === 0) {
    return <div className="placeholder">No comments. Select code and press c.</div>;
  }
  const sendAll = () => review.send(comments.map((c) => commentText(files, c)).join("\n\n---\n\n"));
  return (
    <>
      <div className="tree-total">
        <span>
          {comments.length} {comments.length === 1 ? "comment" : "comments"}
        </span>
        <span className="spacer" />
        <button type="button" onClick={sendAll} title="Paste every comment into the agent's prompt">
          Send all to agent
        </button>
      </div>
      <ul>
        {comments.map((c) => (
          <li key={c.id}>
            <button type="button" onClick={() => review.jumpToComment(c.id)} title={c.path}>
              <span className="comment-where">
                {rowLabel(files, c.file, c.path, c.rows?.end)}
                {!c.rows && <span className="comment-outdated">code changed</span>}
              </span>
              <span className="comment-preview">{preview(c)}</span>
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}
