import type { DockviewPanelApi } from "dockview-react";
import { useEffect, useRef } from "react";
import type { WorkspaceName } from "../workspaces/context";
import { isTarget, publishTarget, type ReviewTarget, retractTarget, targetOf } from "./target";

/** Publishes this pane's target while it's active, or when no pane has, and retracts it when
 * the pane goes. */
export function usePublishTarget(
  api: DockviewPanelApi,
  workspace: WorkspaceName,
  target: Omit<ReviewTarget, "load" | "focus">,
  actions: Pick<ReviewTarget, "load" | "focus">,
) {
  const published = useRef<ReviewTarget>(undefined);
  const latest = useRef(actions);
  latest.current = actions;
  const { revset, changes, loading } = target;

  useEffect(() => {
    const publish = () => {
      published.current = {
        revset,
        changes,
        loading,
        load: (r) => latest.current.load(r),
        focus: () => latest.current.focus(),
      };
      publishTarget(workspace, published.current);
    };
    // Active, unclaimed, or already ours and changed.
    if (api.isActive || !targetOf(workspace) || isTarget(workspace, published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => isActive && publish());
    return () => sub.dispose();
  }, [api, workspace, revset, changes, loading]);

  useEffect(() => () => retractTarget(workspace, published.current), [workspace]);
}
