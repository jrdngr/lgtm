import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { ReviewBox } from "../review/ReviewBox";
import { RevisionsPanel } from "../review/RevisionsPanel";
import { onOpenRevisions, useReviewTarget } from "../review/target";
import { useTrackWorkingCopy, useWorkingCopy } from "../review/workingCopy";
import { contextMenu } from "../ui/contextMenu";
import { revsetMenu } from "./revsetMenu";

/** What the workspace reviews and where `@` is, in the header, whether or not a review pane
 * is open. Opens the revisions panel to pick another review or change commits; `@` in a
 * review opens it too. */
export function RevPicker({ workspace }: { workspace: WorkspaceName }) {
  const target = useReviewTarget(workspace);
  const pr = usePr(workspace);
  const wc = useWorkingCopy(workspace);
  useTrackWorkingCopy(workspace, target);
  const [open, setOpen] = useState(false);
  const [trunk, setTrunk] = useState<string | null>(null);
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

  useEffect(() => {
    let live = true;
    commands.trunkBookmark(workspace).then((r) => live && r.status === "ok" && setTrunk(r.data));
    return () => {
      live = false;
    };
  }, [workspace]);

  const loading = !target || (target.loading && target.changes.length === 0);
  const prReview = pr && target?.revset === pr.revset ? pr : null;

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
        className={`rev-select${open ? " open" : ""}`}
        disabled={!target}
        aria-busy={loading}
        onClick={() => setOpen(true)}
        onContextMenu={contextMenu(() =>
          revsetMenu({ workspace, target, trunk, pr, wc, openRevisions: () => setOpen(true) }),
        )}
        style={loading ? { width: width.current } : undefined}
        title={target?.revset}
      >
        {loading || !target ? (
          <span className="rev-box">
            <span className="rev-shimmer" />
          </span>
        ) : (
          <ReviewBox changes={target.changes} pr={prReview} wc={wc} stale={target.stale} />
        )}
        <kbd>⌘J</kbd>
      </button>
      {open && target && (
        <RevisionsPanel
          workspace={workspace}
          revset={target.revset}
          reviewed={target.changes.map((c) => c.commitId)}
          head={target.changes.at(-1)?.commitId}
          onLoad={target.load}
          onClose={close}
        />
      )}
    </>
  );
}
