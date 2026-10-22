import { type RefObject, useEffect, useRef } from "react";
import { wordAtPoint } from "../search/symbolAt";
import type { LineHandlers } from "./DiffLines";
import { type DisplayLine, type FlatFile, gapKey } from "./model";
import { fileSelection, hunkSelection, rangeSelection, units } from "./selection";
import type { SelectionApi } from "./useSelection";

/** Drops any browser text selection so it doesn't linger next to a line selection. */
export function clearTextSelection() {
  window.getSelection()?.removeAllRanges();
}

/** Where the line menu opens, and the word and file it opened on. */
export interface MenuAt {
  x: number;
  y: number;
  word?: string;
  path?: string;
}

interface Deps {
  files: FlatFile[];
  lines: DisplayLine[];
  s: SelectionApi;
  rootRef: RefObject<HTMLDivElement | null>;
  setMenu(at: MenuAt): void;
  toggleHunk(file: number, hunk: number): void;
  toggleFile(file: number): void;
  setChecked(file: number, checked: boolean): void;
  reveal(gap: string): void;
}

/** Mouse handlers for the diff's lines: click, drag, and multi-click to select. */
export function useRowMouse(d: Deps): LineHandlers {
  const { files, lines, s, rootRef } = d;
  const { sel, select, place } = s;
  const dragging = useRef(false);

  useEffect(() => {
    const up = () => {
      dragging.current = false;
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, []);

  return {
    onRowMouseDown(e: React.MouseEvent, i: number) {
      if (e.button !== 0) return;
      // Only code text is selectable: a double-click there selects a word. Gutters, and
      // double-clicks on them or triple clicks anywhere, pick rows.
      const inCode = e.target instanceof Element && e.target.closest(".code");
      if (e.detail === 2 && inCode) return;
      if (!inCode || e.detail >= 3) {
        e.preventDefault();
        clearTextSelection();
      }
      if (e.detail === 2) {
        const fn = units(files, lines, i).find((u) => u.level === "function");
        if (fn) select(fn, i);
        return;
      }
      if (e.detail >= 3) {
        const line = lines[i];
        if (line.kind !== "file") select(hunkSelection(lines, line.file, line.hunk), i);
        return;
      }
      if (e.shiftKey) {
        select(rangeSelection(s.anchor, i), i);
        return;
      }
      place(i);
      dragging.current = true;
    },
    onRowMouseEnter(i: number) {
      if (dragging.current) select(rangeSelection(s.anchor, i), i);
    },
    onRowContextMenu(e: React.MouseEvent, i: number) {
      e.preventDefault();
      if (i < sel.start || i > sel.end) place(i);
      const root = rootRef.current?.getBoundingClientRect();
      const word = wordAtPoint(e.clientX, e.clientY);
      const path = files[lines[i]?.file]?.dto.path;
      if (root) d.setMenu({ x: e.clientX - root.left, y: e.clientY - root.top, word, path });
    },
    onFileClick(file: number) {
      s.selectUnit(fileSelection(files, lines, file));
    },
    onHunkClick(file: number, hunk: number) {
      s.selectUnit(hunkSelection(lines, file, hunk));
    },
    onToggleHunk: d.toggleHunk,
    onToggleFile: d.toggleFile,
    onCheckFile: d.setChecked,
    onGapClick(file: number, hunk: number, firstRow: number) {
      d.reveal(gapKey(file, hunk, firstRow));
    },
  };
}
