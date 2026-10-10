import type { RowMove } from "../bindings";
import { type DisplayLine, markKey } from "./model";
import { lineSelection, rangeSelection, type Selection } from "./selection";

/** How rows and files of the old review map onto the new one. Absent means unchanged. */
export interface Moves {
  rows: Map<string, string>;
  files: Map<number, number>;
}

export function movesFrom(moves: RowMove[], files: [number, number][]): Moves {
  return {
    rows: new Map(
      moves.map((m) => [markKey(m.from.file, m.from.row), markKey(m.to.file, m.to.row)]),
    ),
    files: new Map(files),
  };
}

/** Keys a line is found by: its rows, or its header. Gap rows are hidden, so they don't count. */
function keys(line: DisplayLine): string[] {
  if (line.kind === "rows") return line.rows.map((r) => markKey(line.file, r));
  if (line.kind === "hunk") return [`h:${line.file}:${line.hunk}`];
  if (line.kind === "file") return [`f:${line.file}`];
  return [];
}

function moveKey(key: string, moves: Moves | undefined): string | undefined {
  if (!moves) return key;
  if (key.startsWith("f:")) {
    const to = moves.files.get(Number(key.slice(2)));
    return to === undefined ? undefined : `f:${to}`;
  }
  if (key.startsWith("h:")) return undefined;
  return moves.rows.get(key);
}

export interface Placement {
  sel: Selection;
  cursor: number;
  anchor: number;
}

/**
 * Carries the selection, cursor, and extend anchor over to a new set of lines. A selection
 * stays on the same rows, shrinks to those that are left, or clears to the cursor when none
 * are. A cursor whose row is gone moves to the next shown row, or the previous one at the end.
 */
export function follow(
  before: DisplayLine[],
  after: DisplayLine[],
  at: Placement,
  moves?: Moves,
): Placement {
  const index = new Map<string, number>();
  after.forEach((line, i) => {
    for (const k of keys(line)) if (!index.has(k)) index.set(k, i);
  });
  const find = (i: number): number | undefined => {
    for (const k of before[i] ? keys(before[i]) : []) {
      const moved = moveKey(k, moves);
      const found = moved === undefined ? undefined : index.get(moved);
      if (found !== undefined) return found;
    }
    return undefined;
  };
  const nearest = (i: number): number => {
    for (let j = i; j < before.length; j++) {
      const found = find(j);
      if (found !== undefined && after[found].kind === "rows") return found;
    }
    for (let j = i - 1; j >= 0; j--) {
      const found = find(j);
      if (found !== undefined) return found;
    }
    const first = after.findIndex((l) => l.kind === "rows");
    return Math.max(first, 0);
  };

  const cursor = nearest(at.cursor);
  const anchor = find(at.anchor) ?? cursor;
  if (at.sel.level === "review") {
    return { sel: { ...at.sel, start: 0, end: Math.max(after.length - 1, 0) }, cursor, anchor };
  }
  const found: number[] = [];
  let lost = false;
  for (let i = at.sel.start; i <= at.sel.end; i++) {
    const f = find(i);
    if (f === undefined) lost ||= before[i]?.kind === "rows";
    else found.push(f);
  }
  if (found.length === 0 || (at.sel.level === "line" && lost)) {
    return { sel: lineSelection(cursor), cursor, anchor: cursor };
  }
  const start = Math.min(...found);
  const end = Math.max(...found);
  const sel =
    lost || end - start !== at.sel.end - at.sel.start
      ? rangeSelection(start, end)
      : { ...at.sel, start, end };
  const inside = cursor >= start && cursor <= end;
  return { sel, cursor: inside ? cursor : start, anchor };
}
