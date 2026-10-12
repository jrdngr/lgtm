import { useEffect, useState } from "react";
import { RevisionsPanel } from "../review/RevisionsPanel";
import { RevLabel } from "../review/RevLabel";
import { onOpenRevisions, useReviewTarget } from "../review/target";
import type { WorkspaceName } from "../workspaces/context";
import "../review/ReviewBar.css";

/** What the workspace reviews, in the header, whether or not a review pane is open. Opens
 * the revisions panel to pick another revset or change commits; `r` in a review opens it
 * too. */
export function RevPicker({ workspace }: { workspace: WorkspaceName }) {
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
        disabled={!target}
        onClick={() => setOpen(true)}
        title="Revisions (r): pick what to review, edit, add and squash commits"
      >
        {!target || (target.loading && target.changes.length === 0) ? (
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
