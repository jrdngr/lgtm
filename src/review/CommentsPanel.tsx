import { useWorkspace } from "../workspaces/context";
import { commentText, rowLabel } from "./comments";
import { useCurrentReview } from "./current";
import "./comments.css";

/** The current review's comments in file order. Click one to jump to it. */
export function CommentsPanel() {
  const review = useCurrentReview(useWorkspace());
  if (!review) return <div className="placeholder">No review open</div>;
  const { comments, files } = review;
  if (comments.length === 0) {
    return <div className="placeholder">No comments. Select code and press c.</div>;
  }
  const sendAll = () => review.send(comments.map((c) => commentText(files, c)).join("\n\n---\n\n"));
  return (
    <div className="comment-list">
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
        {comments.map((c) => {
          return (
            <li key={c.id}>
              <button type="button" onClick={() => review.jumpToComment(c.id)} title={c.path}>
                <span className="comment-where">
                  {rowLabel(files, c.file, c.path, c.rows?.end)}
                  {!c.rows && <span className="comment-outdated">code changed</span>}
                </span>
                <span className="comment-preview">{c.body || "No text yet"}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
