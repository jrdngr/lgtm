import type { SideState } from "./SidePanel";

export interface Sides {
  left: SideState;
  right: SideState;
}

export const DEFAULT_SIDES: Sides = {
  left: { open: true, width: 230, view: "workspaces" },
  right: { open: true, width: 260, view: "files" },
};

const key = (root: string) => `lgtm:sides:${root}`;

/** Side panel state is per repo, shared by its workspaces. */
export function loadSides(root: string): Sides {
  try {
    const saved = JSON.parse(localStorage.getItem(key(root)) ?? "null");
    return {
      left: { ...DEFAULT_SIDES.left, ...saved?.left },
      right: { ...DEFAULT_SIDES.right, ...saved?.right },
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
