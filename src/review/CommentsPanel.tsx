import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { SubmitReview } from "../github/SubmitReview";
import { useGithubOff } from "../tools/agents";
import { Menu } from "../ui/Menu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { CommentList } from "./CommentList";
import { commentText } from "./comments";
import { useCurrentReview } from "./current";
import "./comments.css";

/** The current review's comments by file. In a PR workspace the PR sits on top and the review
 * submit box at the bottom. */
export function CommentsPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  const off = useGithubOff();
  const comments = review?.comments ?? [];
  const viewer = viewerLogin(comments);
  const sendAll = () =>
    review?.send(comments.map((c) => commentText(review.files, c)).join("\n\n---\n\n"));

  return (
    <div className="comments-view">
      <SectionHeader title="Comments" meta={review ? comments.length : undefined}>
        <Menu
          items={[
            {
              label: "Send all to agent",
              title: "Paste every comment into a terminal's prompt",
              onSelect: sendAll,
              disabled: comments.length === 0,
            },
          ]}
        />
      </SectionHeader>
      <div className="comments-scroll">
        {pr && <PrSection pr={pr} review={review} />}
        {review ? <CommentList review={review} viewer={viewer} /> : <Empty>No review open</Empty>}
      </div>
      {pr && review && !off && (
        <SubmitReview
          workspace={workspace}
          comments={comments}
          onSubmitted={() => review.pullPr(true)}
        />
      )}
    </div>
  );
}
