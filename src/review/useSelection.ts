import { useLayoutEffect, useRef, useState } from "react";
import { follow, type Moves } from "./follow";
import type { DisplayLine, FlatFile } from "./model";
import { lineSelection, rangeSelection, type Selection } from "./selection";

export function nextRowLine(lines: DisplayLine[], from: number, dir: 1 | -1): number {
  for (let i = from + dir; i >= 0 && i < lines.length; i += dir) {
    if (lines[i].kind === "rows") return i;
  }
  return from;
}

/**
 * Cursor, selection, and extend anchor over `lines`. When the lines change (view mode,
 * collapse, filter, marks, or a refreshed diff) they follow the rows they were on.
 */
export function useSelection(files: FlatFile[], lines: DisplayLine[], moves: Moves | undefined) {
  const [cursor, setCursor] = useState(0);
  const [sel, setSel] = useState<Selection>(lineSelection(0));
  const [anchor, setAnchor] = useState(0);
  const [shrinkStack, setShrinkStack] = useState<Selection[]>([]);
  const [extending, setExtending] = useState(false);
  const placed = useRef({ files, lines });

  useLayoutEffect(() => {
    const before = placed.current;
    placed.current = { files, lines };
    if (before.lines === lines) return;
    const refreshed = before.files !== files;
    // A newly opened review places its own cursor.
    if (refreshed && !moves) return;
    const next = follow(
      before.lines,
      lines,
      { sel, cursor, anchor },
      refreshed ? moves : undefined,
    );
    setSel(next.sel);
    setCursor(next.cursor);
    setAnchor(next.anchor);
    setShrinkStack([]);
  }, [files, lines, moves, sel, cursor, anchor]);

  function select(next: Selection, at: number = cursor, keepStack = false) {
    setSel(next);
    setCursor(at);
    if (!keepStack) setShrinkStack([]);
  }

  /** Puts the cursor on line `i` and clears the selection to it. */
  function place(i: number) {
    setAnchor(i);
    select(lineSelection(i), i);
  }

  function move(dir: 1 | -1, extend: boolean) {
    const next = nextRowLine(lines, cursor, dir);
    if (extend) select(rangeSelection(anchor, next), next);
    else place(next);
  }

  return {
    cursor,
    sel,
    anchor,
    setAnchor,
    shrinkStack,
    setShrinkStack,
    extending,
    setExtending,
    select,
    place,
    move,
  };
}
