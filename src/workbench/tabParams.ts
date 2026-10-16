import type { DockviewPanelApi } from "dockview-react";

/** Tab state waiting to be saved, by tab. */
const pending = new WeakMap<DockviewPanelApi, { values: Record<string, unknown>; timer: number }>();

/** Saves `values` in the tab's parameters once they stop changing, so they're kept with the
 * layout and come back on restart. Parameters merge, so each caller owns its own keys. */
export function saveTabParams(api: DockviewPanelApi, values: Record<string, unknown>) {
  const waiting = pending.get(api);
  if (waiting) clearTimeout(waiting.timer);
  const merged = { ...waiting?.values, ...values };
  const timer = window.setTimeout(() => {
    pending.delete(api);
    api.updateParameters(merged);
  }, 400);
  pending.set(api, { values: merged, timer });
}
