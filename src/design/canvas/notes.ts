import type { CanvasNote, NoteSize } from "../../bindings";
import type { Box } from "../../system/layout";

export const NOTE_PX: Record<NoteSize, number> = { s: 12, m: 16, l: 22 };

/** About the box a note's text takes. */
export function noteBox(n: CanvasNote): Box {
  const px = NOTE_PX[n.size];
  const lines = n.text.split("\n");
  const longest = Math.max(1, ...lines.map((l) => l.length));
  return {
    x: n.at.x,
    y: n.at.y,
    w: Math.max(40, longest * px * 0.58 + 8),
    h: lines.length * px * 1.35 + 4,
  };
}
