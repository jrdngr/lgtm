import type { CanvasNote, NoteSize } from "../bindings";
import type { Box } from "../system/layout";

export const NOTE_PX: Record<NoteSize, number> = { s: 12, m: 16, l: 22 };
/** A sticky note's width, by size. */
export const STICKY_W: Record<NoteSize, number> = { s: 160, m: 220, l: 300 };
/** Inside a sticky note, around its text. */
export const STICKY_PAD = 12;

/** About how wide a character of `px` text draws. */
const CHAR = 0.58;

/** About the box a note takes: its text's, or its sticky card's. */
export function noteBox(n: Pick<CanvasNote, "at" | "text" | "size" | "sticky">): Box {
  const px = NOTE_PX[n.size];
  const lines = n.text.split("\n");
  if (n.sticky) {
    const w = STICKY_W[n.size];
    const perLine = Math.max(1, Math.floor((w - STICKY_PAD * 2) / (px * CHAR)));
    const rows = lines.reduce((s, l) => s + Math.max(1, Math.ceil(l.length / perLine)), 0);
    return { x: n.at.x, y: n.at.y, w, h: rows * px * 1.35 + STICKY_PAD * 2 };
  }
  const longest = Math.max(1, ...lines.map((l) => l.length));
  return {
    x: n.at.x,
    y: n.at.y,
    w: Math.max(40, longest * px * CHAR + 8),
    h: lines.length * px * 1.35 + 4,
  };
}
