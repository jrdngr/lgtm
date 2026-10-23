import { useState } from "react";
import type { Motion, MotionTarget } from "./motions";

export interface RevPick {
  motion: Motion;
  target: MotionTarget;
  /** The log rows to pick from, top to bottom, and the change each is of. */
  rows: { row: number; change: string }[];
  /** The highlighted row. */
  at: number;
}

/** Runs a motion on the commit the user picks from `pick`, in place of the cursor. */
export function useRevPick(run: (m: Motion, t: MotionTarget) => void) {
  const [pick, setPick] = useState<RevPick>();

  /** Opens the pick on `rows`, highlighting the cursor's commit. */
  function start(motion: Motion, target: MotionTarget, rows: RevPick["rows"]) {
    const at = rows.findIndex((r) => r.change === target.cursor);
    setPick({ motion, target, rows, at: Math.max(at, 0) });
  }

  function choose(i: number) {
    const row = pick?.rows[i];
    if (!pick || !row) return;
    setPick(undefined);
    run(pick.motion, { ...pick.target, cursor: row.change });
  }

  /** Takes every key while picking. Returns whether a pick is open. */
  function key(e: React.KeyboardEvent): boolean {
    if (!pick) return false;
    e.preventDefault();
    e.stopPropagation();
    const n = pick.rows.length;
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
