import type { DockviewApi } from "dockview-react";
import { useSyncExternalStore } from "react";

/** Tabs picked with ⌘-click, by panel id, to move together. */
let picked = new Set<string>();
const listeners = new Set<() => void>();
const set = (next: Set<string>) => {
  picked = next;
  for (const l of listeners) l();
};

export function useTabPicked(id: string): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => picked.has(id),
  );
}

/** Adds tab `id` to the picked ones, or takes it out. The first pick also takes `active`, the
 * tab shown in its pane. */
export function togglePick(id: string, active: string | undefined) {
  const next = new Set(picked);
  if (next.has(id)) next.delete(id);
  else {
    if (!next.size && active && active !== id) next.add(active);
    next.add(id);
  }
  set(next);
}

export function clearPicks() {
  if (picked.size) set(new Set());
}

/** The picked tabs, when `id` is one of several picked. */
export function pickedWith(id: string): string[] {
  return picked.has(id) && picked.size > 1 ? [...picked] : [];
}

/** Dragging one picked tab to another pane of `api` takes the other picked tabs of the dock
 * there too. */
export function moveTogether(api: DockviewApi) {
  let moving = false;
  return api.onDidMovePanel((e) => {
    if (moving || e.from === e.to) return;
    const rest = pickedWith(e.panel.id).filter((id) => id !== e.panel.id);
    if (!rest.length) return;
    moving = true;
    try {
      for (const id of rest) {
        const p = api.getPanel(id);
        if (p && p.group !== e.to) p.api.moveTo({ group: e.to, position: "center" });
      }
      e.panel.api.setActive();
    } finally {
      moving = false;
    }
    clearPicks();
  });
}
