import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { editAction } from "../review/RevisionsPanelMenu";
import { useReviewTarget } from "../review/target";
import { useAtOutside } from "../review/workingCopy";
import { run } from "../services/pathMenu";
import { actOn, reviewRevset } from "./revsetMenu";

/** Under the header while `@` is outside the review: review `@`, or edit the review's head. */
export function AtBanner({ workspace }: { workspace: WorkspaceName }) {
  const target = useReviewTarget(workspace);
  const pr = usePr(workspace);
  const outside = useAtOutside(workspace);
  const head = target?.changes.at(-1);
  if (!outside || !target) return null;
  const onPr = pr && target.revset === pr.revset;
  return (
    <div className="at-banner">
      <span className="rev-at">@</span>
      <span>@ is outside the review</span>
      <span className="spacer" />
      <button type="button" onClick={() => reviewRevset(target, "@")}>
        Review @
      </button>
      {head && (
        <button
          type="button"
          disabled={target.loading}
          onClick={() => run(actOn(workspace, target, editAction(head.changeId)))}
        >
          {onPr ? `Edit #${pr.number} Head` : "Edit Review Head"}
        </button>
      )}
    </div>
  );
}
