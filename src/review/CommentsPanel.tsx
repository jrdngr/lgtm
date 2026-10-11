import type { CommentDto } from "../bindings";
import { usePr } from "../github/pr";
import { SubmitReview } from "../github/SubmitReview";
import { useGithubOff } from "../tools/agents";
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

/** Submitting the review and pulling threads, in a PR workspace. */
function PrBar({ review }: { review: CurrentReview }) {
  const workspace = useWorkspace();
  const pr = usePr(workspace);
  const off = useGithubOff();
  if (!pr || off) return null;
  return (
    <>
      <div className="tree-total">
        <span>PR #{pr.number}</span>
        <span className="spacer" />
        <button
          type="button"
          onClick={() => review.pullPr()}
          title="Pull the PR's threads from GitHub"
        >
          Pull
        </button>
      </div>
      <SubmitReview
        workspace={workspace}
        comments={review.comments}
        onSubmitted={() => review.pullPr(true)}
      />
    </>
  );
}

/** The current review's comments in file order. Click one to jump to it. */
export function CommentsPanel() {
  const review = useCurrentReview(useWorkspace());
  if (!review) return <div className="placeholder">No review open</div>;
  const { comments, files } = review;
  if (comments.length === 0) {
    return (
      <>
        <PrBar review={review} />
        <div className="placeholder">No comments. Select code and press c.</div>
      </>
    );
  }
  const sendAll = () => review.send(comments.map((c) => commentText(files, c)).join("\n\n---\n\n"));
  return (
    <div className="comment-list">
      <PrBar review={review} />
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
                <span className="comment-preview">{preview(c)}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
