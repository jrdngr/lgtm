import type { WorkspaceName } from "../core/workspace";
import type { SideState } from "./SidePanel";
import { movedView } from "./subviews";

export interface Sides {
  left: SideState;
  right: SideState;
  /** The right panel's view each workspace was left on, by workspace key. */
  rightViews?: Record<string, string>;
}

const DEFAULT_SIDES: Sides = {
  left: { open: true, width: 230, view: "workspaces" },
  right: { open: true, width: 260, view: "changes" },
};

const key = (root: string) => `lgtm:sides:${root}`;

/** Side panel state is per repo, shared by its workspaces, except the right panel's view,
 * which each workspace keeps. */
export function loadSides(root: string): Sides {
  try {
    const saved = JSON.parse(localStorage.getItem(key(root)) ?? "null");
    const right: SideState = { ...DEFAULT_SIDES.right, ...saved?.right };
    const split = right.split && { ...right.split, view: movedView(right.split.view) };
    return {
      left: { ...DEFAULT_SIDES.left, ...saved?.left },
      right: { ...right, view: movedView(right.view), split },
      rightViews: saved?.rightViews,
    };
  } catch {
    return DEFAULT_SIDES;
  }
}

export function saveSides(root: string, sides: Sides) {
  try {
    localStorage.setItem(key(root), JSON.stringify(sides));
  } catch {
    // Remembering panel state is a convenience.
  }
}

const selectedKey = (root: string) => `lgtm:workspace:${root}`;

/** The workspace last selected in the repo. */
export function loadWorkspace(root: string): WorkspaceName {
  try {
    return localStorage.getItem(selectedKey(root)) || null;
  } catch {
    return null;
  }
}

export function saveWorkspace(root: string, name: WorkspaceName) {
  try {
    localStorage.setItem(selectedKey(root), name ?? "");
  } catch {
    // Remembering the workspace is a convenience.
  }
}

/** `sides` with the right panel on the view workspace `ws` was left on. */
export function rightViewOf(sides: Sides, ws: string): Sides {
  const view = sides.rightViews?.[ws];
  return view ? { ...sides, right: { ...sides.right, view: movedView(view) } } : sides;
}

/** `sides` remembering the right panel's view as workspace `ws`'s. */
export function keepRightView(sides: Sides, ws: string): Sides {
  return { ...sides, rightViews: { ...sides.rightViews, [ws]: sides.right.view } };
}
