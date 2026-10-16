import { useState } from "react";
import { type CurrentReview, useCurrentReview } from "../../review/current";
import { Busy } from "../../ui/Busy";
import { Empty, SectionHeader } from "../../ui/SectionHeader";
import { useWorkspace, type WorkspaceName } from "../../workspaces/context";
import { useArchRun } from "../runs";
import { useArch } from "../state";
import { History } from "./History";
import { Intent } from "./Intent";
import { Layers } from "./Layers";
import { Look } from "./Look";
import { Score } from "./Score";
import "./sidebar.css";

/** The current review's architecture: Assess's score, the change stack, what to look at, your
 * intent marks, and the commits that changed it. */
export function ArchSection() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  return (
    <div className="side-split">
      <SectionHeader title="Architecture" />
      {review ? (
        <Body key={review.id} workspace={workspace} review={review} />
      ) : (
        <Empty>No review open</Empty>
      )}
    </div>
  );
}

function Body({ workspace, review }: { workspace: WorkspaceName; review: CurrentReview }) {
  const { arch, error, view } = useArch(review);
  const assess = useArchRun("assess", review.id);
  const [actionError, setActionError] = useState<string>();
  return (
    <div className="side-split-body arch-side">
      <Score
        workspace={workspace}
        review={review.id}
        revset={review.revset}
        assessment={arch?.assessment}
        run={assess}
      />
      {(error || actionError) && <div className="note-run error">{error ?? actionError}</div>}
      {!arch ? (
        !error && (
          <div className="placeholder arch-loading">
            <Busy>Loading…</Busy>
          </div>
        )
      ) : (
        <>
          <div className={assess?.running ? "arch-stale" : undefined}>
            <Layers
              workspace={workspace}
              review={review}
              layers={arch.layers}
              assessed={!!arch.assessment}
              selection={view.selection}
            />
            <Look
              workspace={workspace}
              review={review.id}
              look={arch.look}
              selection={view.selection}
              onError={setActionError}
            />
          </div>
          <Intent
            workspace={workspace}
            review={review}
            intents={arch.intents}
            components={arch.components}
            selection={view.selection}
            onError={setActionError}
          />
        </>
      )}
      <History review={review} view={view} />
    </div>
  );
}
