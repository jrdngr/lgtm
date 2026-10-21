import { useEffect, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useReviewTarget } from "../review/target";
import { useWorkingCopy } from "../review/workingCopy";
import { contextMenu } from "../ui/contextMenu";
import { SectionHeader } from "../ui/SectionHeader";
import { revsetMenu } from "../workbench/revsetMenu";
import { RevisionsLog } from "./RevisionsLog";

interface Props {
  workspace: WorkspaceName;
  /** Changes when the user asks for a refresh. */
  refreshed: number;
}

/** The selected workspace's revisions, under its header and menu. */
export function RevisionsSection({ workspace, refreshed }: Props) {
  const target = useReviewTarget(workspace);
  const pr = usePr(workspace);
  const wc = useWorkingCopy(workspace);
  const [trunk, setTrunk] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    commands.trunkBookmark(workspace).then((r) => live && r.status === "ok" && setTrunk(r.data));
    return () => {
      live = false;
    };
  }, [workspace]);

  return (
    <div
      className="revisions-section"
      onContextMenu={contextMenu(() => revsetMenu({ workspace, target, trunk, pr, wc }))}
    >
      <SectionHeader title="Revisions">
        {target?.stale && <span className="pill amber">stale</span>}
      </SectionHeader>
      <RevisionsLog
        key={String(workspace)}
        workspace={workspace}
        refreshed={refreshed}
        trunk={trunk}
      />
    </div>
  );
}
