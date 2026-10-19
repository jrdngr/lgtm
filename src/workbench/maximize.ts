import type { DockviewApi, DockviewGroupPanel } from "dockview-react";
import { useEffect, useReducer } from "react";
import { unfold } from "./fold";

/** Whether `group` can fill the dock: it's maximized, or has other panes to hide. */
export const canMaximize = (api: DockviewApi, group: DockviewGroupPanel) =>
  group.api.isMaximized() || api.groups.length > 1;

/** Fills the dock with `group`, hiding the other panes, or brings them all back when a pane
 * already fills it. Collapsed panes stay collapsed; a collapsed group opens first to fill. */
export function toggleMaximize(api: DockviewApi, group: DockviewGroupPanel | undefined) {
  if (api.hasMaximizedGroup()) {
    api.exitMaximizedGroup();
    return;
  }
  if (!group || !canMaximize(api, group)) return;
  unfold(api, group);
  group.api.maximize();
}

/** Re-renders when a pane is maximized or restored, or panes come and go. */
export function useMaximized(api: DockviewApi | undefined) {
  const [, tick] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (!api) return;
    const subs = [api.onDidMaximizedGroupChange(tick), api.onDidLayoutChange(tick)];
    return () => {
      for (const s of subs) s.dispose();
    };
  }, [api]);
  return !!api?.hasMaximizedGroup();
}
