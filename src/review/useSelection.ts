import { useLayoutEffect, useRef, useState } from "react";
import { follow, type Moves } from "./follow";
import { type DisplayLine, type FlatFile, nextRowLine } from "./model";
import { lineSelection, rangeSelection, type Selection } from "./selection";

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

  /** Selects a whole unit, with the cursor and anchor at its start. */
  function selectUnit(next: Selection) {
    setAnchor(next.start);
    select(next, next.start);
  }

  function move(dir: 1 | -1, extend: boolean, from = anchor) {
    const next = nextRowLine(lines, cursor, dir);
    if (extend) select(rangeSelection(from, next), next);
    else place(next);
  }

  /** Widens to `next`, remembering the current selection for `shrink`. */
  function grow(next: Selection) {
    setShrinkStack([...shrinkStack, sel]);
    select(next, cursor, true);
  }

  /** Goes back to the selection before the last `grow`. */
  function shrink() {
    const prev = shrinkStack.at(-1);
    if (!prev) return;
    setShrinkStack(shrinkStack.slice(0, -1));
    select(prev, cursor, true);
  }

  /** Extends the selection one row, from the cursor unless it's already a range. */
  function extend(dir: 1 | -1) {
    const from = !extending && sel.level !== "range" ? cursor : anchor;
    setAnchor(from);
    move(dir, true, from);
  }

  function toggleExtend() {
    setExtending((x) => !x);
    setAnchor(cursor);
  }

  /** Stops extending and clears the selection to the cursor. */
  function collapse() {
    setExtending(false);
    place(cursor);
  }

  return {
    cursor,
    sel,
    anchor,
    extending,
    select,
    selectUnit,
    place,
    move,
    grow,
    shrink,
    extend,
    toggleExtend,
    collapse,
  };
}

export type SelectionApi = ReturnType<typeof useSelection>;
