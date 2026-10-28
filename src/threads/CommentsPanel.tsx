import { useWorkspace } from "../core/workspace";
import { usePr } from "../github/pr";
import { useCurrentReview } from "../review/current";
import { Empty } from "../ui/SectionHeader";
import { PrComments } from "./PrComments";
import "../review/comments.css";
import "./threads.css";

/** The workspace's comments: the PR's threads, in a PR workspace. */
export function CommentsPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  return (
    <div className="comments-view">
      {pr ? <PrComments pr={pr} review={review} /> : <Empty>No PR</Empty>}
    </div>
  );
}
