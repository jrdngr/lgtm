import { useWorkspace } from "../core/workspace";
import { usePr } from "../github/pr";
import { useCurrentReview } from "../review/current";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { AgentCommentList, ResolvedToggle } from "./AgentCommentList";
import { PrComments } from "./PrComments";
import "../review/comments.css";
import "./threads.css";

/** The workspace's comments: those for the agent, then the PR's threads in a PR workspace. */
export function CommentsPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  return (
    <div className="comments-view">
      {pr ? (
        <PrComments pr={pr} review={review} />
      ) : (
        <>
          <SectionHeader title="Comments">
            <ResolvedToggle />
          </SectionHeader>
          <div className="comments-scroll">
            {!review ? (
              <Empty>No review open</Empty>
            ) : review.agentComments.length ? (
              <AgentCommentList review={review} />
            ) : (
              <Empty>No comments</Empty>
            )}
          </div>
        </>
      )}
    </div>
  );
}
