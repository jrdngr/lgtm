import { useState } from "react";
import type { BookmarkSpot } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";
import type { Motion, MotionTarget } from "./motions";

export interface BookmarkPick {
  motion: Motion;
  target: MotionTarget;
  spots: BookmarkSpot[];
  /** The highlighted row. */
  at: number;
}

/** Runs a motion that moves a bookmark: at once when only one is near the cursor, otherwise
 * after the user picks one from `pick`. */
export function useBookmarkPick(
  workspace: WorkspaceName,
  run: (m: Motion, t: MotionTarget) => void,
  onError: (message: string) => void,
) {
  const [pick, setPick] = useState<BookmarkPick>();

  async function start(motion: Motion, target: MotionTarget) {
    const r = await commands.nearestBookmarks(workspace, target.cursor);
    if (r.status === "error") return onError(r.error);
    const spots = r.data;
    if (!spots.length) return onError("No bookmark nearby");
    if (spots.length === 1) return run(motion, { ...target, bookmark: spots[0].name });
    setPick({ motion, target, spots, at: 0 });
  }

  function choose(i: number) {
    const spot = pick?.spots[i];
    if (!pick || !spot) return;
    setPick(undefined);
    run(pick.motion, { ...pick.target, bookmark: spot.name });
  }

  /** Takes every key while picking. Returns whether a pick is open. */
  function key(e: React.KeyboardEvent): boolean {
    if (!pick) return false;
    e.preventDefault();
    e.stopPropagation();
    const n = pick.spots.length;
    const digit = Number(e.key);
    if (e.key === "Escape") setPick(undefined);
    else if (e.key === "Enter") choose(pick.at);
    else if (Number.isInteger(digit) && digit >= 1) choose(digit - 1);
    else if (e.key === "ArrowDown" || e.key === "j") setPick({ ...pick, at: (pick.at + 1) % n });
    else if (e.key === "ArrowUp" || e.key === "k") setPick({ ...pick, at: (pick.at + n - 1) % n });
    return true;
  }

  return { pick, start, choose, key };
}
