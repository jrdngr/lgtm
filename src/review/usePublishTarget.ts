import type { DockviewPanelApi } from "dockview-react";
import { useEffect, useRef } from "react";
import { inDock } from "../workbench/docks";
import type { WorkspaceName } from "../workspaces/context";
import { isIdle } from "./idle";
import {
  isTarget,
  onTargetChange,
  publishTarget,
  type ReviewTarget,
  retractTarget,
  targetOf,
} from "./target";

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
      // A pane left outside the dock can't show anything.
      if (!inDock(workspace, api.id)) return;
      published.current = {
        revset,
        changes,
        loading,
        load: (r) => latest.current.load(r),
        focus: () => latest.current.focus(),
      };
      publishTarget(workspace, published.current);
    };
    // Active, unclaimed or held by the idle review, or already ours and changed.
    const free = !targetOf(workspace) || isIdle(targetOf(workspace));
    if (api.isActive || free || isTarget(workspace, published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => isActive && publish());
    // Takes over when the pane that was the target closes.
    const off = onTargetChange(() => {
      if (!targetOf(workspace)) publish();
    });
    return () => {
      sub.dispose();
      off();
    };
  }, [api, workspace, revset, changes, loading]);

  useEffect(() => () => retractTarget(workspace, published.current), [workspace]);
}
