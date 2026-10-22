import { useCallback, useEffect, useRef } from "react";
import type { WorkingCopyDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import type { ReviewTarget } from "./target";
import { workspaceStore } from "./workspaceStore";

/** Each workspace's `@` and where it sits against the review, for the header, the outside
 * banner and editor tabs. */
const store = workspaceStore<WorkingCopyDto>();

export const useWorkingCopy = store.use;

/** Whether the workspace's `@` is outside its review, so edits land where nobody's looking. */
export const useAtOutside = (workspace: WorkspaceName) =>
  useWorkingCopy(workspace)?.place === "outside";

/** Keeps the workspace's working copy current as its review's revset and the repo change. */
export function useTrackWorkingCopy(workspace: WorkspaceName, target: ReviewTarget | undefined) {
  const revset = target?.revset;
  const asked = useRef(0);
  const load = useCallback(async () => {
    if (!revset) return;
    const n = ++asked.current;
    const r = await commands.workingCopy(workspace, revset);
    if (n === asked.current && r.status === "ok") store.publish(workspace, r.data);
  }, [workspace, revset]);
  // The review's commits change only with the repo, which reloads this anyway.
  useEffect(() => {
    load();
  }, [load]);
  useRepoChanged(load, workspace);
}
