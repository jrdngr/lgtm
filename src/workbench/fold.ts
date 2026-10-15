import type { DockviewApi, DockviewGroupPanel, IDockviewPanel } from "dockview-react";

/** A collapsed group: the axis it folded along and the size to open it back to. */
export interface Fold {
  axis: "x" | "y";
  size: number;
}

/** How thick a collapsed group's strip is. */
export const STRIP = 28;
/** Dockview's own smallest group. */
const MIN = 100;

const folds = new WeakMap<DockviewApi, Map<string, Fold>>();
const listeners = new Set<() => void>();
const changed = () => {
  for (const l of listeners) l();
};

export function subscribeFolds(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

const foldsOf = (api: DockviewApi) => {
  let m = folds.get(api);
  if (!m) {
    m = new Map();
    folds.set(api, m);
  }
  return m;
};

export const foldOf = (api: DockviewApi, group: DockviewGroupPanel) =>
  folds.get(api)?.get(group.id);

type Dir = "left" | "right" | "up" | "down";

/** The group next to `group` toward `dir`. */
export const neighbor = (api: DockviewApi, group: DockviewGroupPanel, dir: Dir) =>
  api.adjacentGroupInDirection(group, dir) as DockviewGroupPanel | undefined;

/** The axis a group folds along: across its side-by-side neighbors, else its stacked ones.
 * It folds only next to an open neighbor, which takes the space. */
export function foldAxis(api: DockviewApi, group: DockviewGroupPanel): Fold["axis"] | undefined {
  const open = (dir: Dir) => {
    const next = neighbor(api, group, dir);
    return next && !foldOf(api, next);
  };
  if (open("left") || open("right")) return "x";
  if (open("up") || open("down")) return "y";
  return undefined;
}

function pin(group: DockviewGroupPanel, axis: Fold["axis"], size: number | undefined) {
  const min = size ?? MIN;
  const max = size ?? Number.POSITIVE_INFINITY;
  group.api.setConstraints(
    axis === "x"
      ? { minimumWidth: min, maximumWidth: max }
      : { minimumHeight: min, maximumHeight: max },
  );
  if (size !== undefined) group.api.setSize(axis === "x" ? { width: size } : { height: size });
  group.element.classList.toggle("folded", size !== undefined);
  group.element.classList.toggle("folded-y", size !== undefined && axis === "y");
}

/** Shrinks `group` to a strip, keeping its tabs. */
export function fold(api: DockviewApi, group: DockviewGroupPanel, axis = foldAxis(api, group)) {
  if (!axis || foldOf(api, group)) return;
  const size = axis === "x" ? group.api.width : group.api.height;
  foldsOf(api).set(group.id, { axis, size: Math.max(size, MIN) });
  pin(group, axis, STRIP);
  changed();
}

/** Opens a folded group back to its size. */
export function unfold(api: DockviewApi, group: DockviewGroupPanel) {
  const f = foldOf(api, group);
  if (!f) return;
  folds.get(api)?.delete(group.id);
  pin(group, f.axis, undefined);
  group.api.setSize(f.axis === "x" ? { width: f.size } : { height: f.size });
  changed();
}

/** The folded groups, to save with the layout. */
export const savedFolds = (api: DockviewApi) => Object.fromEntries(folds.get(api) ?? []);

/** Folds the groups a saved layout had folded, and opens a group whenever one of its tabs
 * is picked or a tab lands in it. */
export function trackFolds(api: DockviewApi, saved: Record<string, Fold> | undefined) {
  // Dockview types groups it hands out as the interface; they're always the class.
  for (const [id, f] of Object.entries(saved ?? {})) {
    const group = api.getGroup(id) as DockviewGroupPanel | undefined;
    if (!group) continue;
    foldsOf(api).set(id, f);
    pin(group, f.axis, STRIP);
  }
  if (saved) changed();
  const groupOf = (panel: IDockviewPanel) => panel.group as DockviewGroupPanel;
  api.onDidActivePanelChange(({ panel }) => {
    if (panel) unfold(api, groupOf(panel));
  });
  api.onDidAddPanel((panel) => unfold(api, groupOf(panel)));
  api.onDidRemoveGroup((group) => {
    if (folds.get(api)?.delete(group.id)) changed();
  });
}
