import type { CurrentReview } from "../review/current";
import { useGithubOff } from "../tools/agents";
import { prOpen } from "../workbench/subviews";
import { useWorkspace } from "../workspaces/context";
import { PrDiscussion } from "./PrDiscussion";
import { usePr } from "./pr";
import { SubmitReview } from "./SubmitReview";

/** A PR workspace's PR atop its comments: the conversation, which folds away, pulling threads,
 * and submitting the review. */
export function PrSection({ review }: { review: CurrentReview | undefined }) {
  const workspace = useWorkspace();
  const pr = usePr(workspace);
  const off = useGithubOff();
  const github = review && !off;
  const open = prOpen.use();
  if (!pr) return null;
  return (
    <>
      <div className="tree-total">
        <button
          type="button"
          className="pr-toggle"
          aria-expanded={open}
          onClick={() => prOpen.set(!open)}
          title={open ? "Hide the PR's conversation" : "Show the PR's conversation"}
        >
          <span className="chevron">{open ? "▾" : "▸"}</span>
          PR #{pr.number}
        </button>
        <span className="spacer" />
        {github && (
          <button
            type="button"
            onClick={() => review.pullPr()}
            title="Pull the PR's threads from GitHub"
          >
            Pull
          </button>
        )}
      </div>
      {open && <PrDiscussion pr={pr} />}
      {github && (
        <SubmitReview
          workspace={workspace}
          comments={review.comments}
          onSubmitted={() => review.pullPr(true)}
        />
      )}
    </>
  );
}
