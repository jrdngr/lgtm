import { useEffect, useState } from "react";
import { RevisionsPanel } from "../review/RevisionsPanel";
import { RevLabel } from "../review/RevLabel";
import { onOpenRevisions, useReviewTarget } from "../review/target";
import type { WorkspaceName } from "../workspaces/context";
import "../review/ReviewBar.css";

interface Props {
  workspace: WorkspaceName;
  /** Opens a review pane, for when the workspace has none. */
  onNoReview(): void;
}

/** What the workspace's review pane reviews, in the header. Opens the revisions panel to
 * pick another revset or change commits; `r` in a review opens it too. */
export function RevPicker({ workspace, onNoReview }: Props) {
  const target = useReviewTarget(workspace);
  const [open, setOpen] = useState(false);

  useEffect(
    () =>
      onOpenRevisions((w) => {
        if (w === workspace) setOpen(true);
      }),
    [workspace],
  );

  function close() {
    setOpen(false);
    target?.focus();
  }

  return (
    <>
      <button
        type="button"
        className={`rev-chip rev-select${open ? " open" : ""}`}
        onClick={() => (target ? setOpen(true) : onNoReview())}
        title="Revisions (r): pick what to review, edit, add and squash commits"
      >
        {!target ? (
          <span className="rev-select-empty">Open a review</span>
        ) : target.loading && target.changes.length === 0 ? (
          <span className="rev-select-empty">Loading…</span>
        ) : (
          <RevLabel revset={target.revset} changes={target.changes} />
        )}
        <span className="rev-select-caret" aria-hidden="true">
          ▾
        </span>
      </button>
      {open && target && (
        <RevisionsPanel
          workspace={workspace}
          revset={target.revset}
          head={target.changes.at(-1)?.commitId}
          onLoad={target.load}
          onClose={close}
        />
      )}
    </>
  );
}
