import { useEffect, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useReviewTarget } from "../review/target";
import { useWorkingCopy } from "../review/workingCopy";
import { contextMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { SectionHeader } from "../ui/SectionHeader";
import { revsetMenu } from "../workbench/revsetMenu";
import { useLogRevset } from "./logRevset";
import { RevisionsLog } from "./RevisionsLog";
import { RevsetStrip } from "./RevsetStrip";

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
  const [editing, setEditing] = useState(false);
  const custom = useLogRevset(workspace)?.revset != null;

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
        <IconButton
          icon="filter"
          label="Revset"
          className={custom ? "on" : undefined}
          aria-pressed={editing}
          onClick={() => setEditing(!editing)}
        />
      </SectionHeader>
      <RevsetStrip
        key={String(workspace)}
        workspace={workspace}
        editing={editing}
        setEditing={setEditing}
      />
      <RevisionsLog
        key={String(workspace)}
        workspace={workspace}
        refreshed={refreshed}
        trunk={trunk}
      />
    </div>
  );
}
