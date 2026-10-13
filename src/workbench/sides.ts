import type { WorkspaceName } from "../workspaces/context";
import type { SideState } from "./SidePanel";
import { movedView } from "./subviews";

export interface Sides {
  left: SideState;
  right: SideState;
}

const DEFAULT_SIDES: Sides = {
  left: { open: true, width: 230, view: "workspaces" },
  right: { open: true, width: 260, view: "changes" },
};

const key = (root: string) => `lgtm:sides:${root}`;

/** Side panel state is per repo, shared by its workspaces. */
export function loadSides(root: string): Sides {
  try {
    const saved = JSON.parse(localStorage.getItem(key(root)) ?? "null");
    const right = { ...DEFAULT_SIDES.right, ...saved?.right };
    return {
      left: { ...DEFAULT_SIDES.left, ...saved?.left },
      right: { ...right, view: movedView(right.view) },
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
