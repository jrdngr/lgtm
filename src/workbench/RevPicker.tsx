import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { RevisionsPanel } from "../review/RevisionsPanel";
import { RevLabel } from "../review/RevLabel";
import { onOpenRevisions, type ReviewTarget, useReviewTarget } from "../review/target";
import type { WorkspaceName } from "../workspaces/context";
import "../review/ReviewBar.css";

/** The full revset, its change count, and staleness, over the field. */
function hover({ revset, changes, stale }: ReviewTarget) {
  const n = changes.length;
  const count = `${n} change${n === 1 ? "" : "s"}`;
  return `${revset} · ${count}${stale ? " · working copy is stale" : ""}\nRevisions (r)`;
}

/** What the workspace reviews, in the header, whether or not a review pane is open. Opens
 * the revisions panel to pick another revset or change commits; `r` in a review opens it
 * too. Amber warns of a change with no description or a stale working copy. */
export function RevPicker({ workspace }: { workspace: WorkspaceName }) {
  const target = useReviewTarget(workspace);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLButtonElement>(null);
  /** The field's width with a label in it, kept while the next review loads. */
  const width = useRef<number>(undefined);

  useEffect(
    () =>
      onOpenRevisions((w) => {
        if (w === workspace) setOpen(true);
      }),
    [workspace],
  );

  const loading = !target || (target.loading && target.changes.length === 0);
  const head = target?.changes.at(-1);
  const warn = !loading && (target?.stale || (head && !head.description));

  useLayoutEffect(() => {
    if (!loading && ref.current) width.current = ref.current.offsetWidth;
  });

  function close() {
    setOpen(false);
    target?.focus();
  }

  return (
    <>
      <button
        ref={ref}
        type="button"
        className={`rev-chip rev-select${open ? " open" : ""}${warn ? " warn" : ""}`}
        disabled={!target}
        aria-busy={loading}
        onClick={() => setOpen(true)}
        style={loading ? { width: width.current } : undefined}
        title={loading || !target ? "Revisions (r)" : hover(target)}
      >
        {loading || !target ? (
          <span className="rev-shimmer" />
        ) : (
          <RevLabel revset={target.revset} changes={target.changes} />
        )}
        <kbd>r</kbd>
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
