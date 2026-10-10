import { type RefObject, useEffect, useRef } from "react";
import type { LineHandlers } from "./DiffLines";
import { type DisplayLine, type FlatFile, gapKey, type SymbolMarkers, symbolRange } from "./model";
import { fileSelection, hunkSelection, rangeSelection, units } from "./selection";
import type { SelectionApi } from "./useSelection";

/** Drops any browser text selection so it doesn't linger next to a line selection. */
export function clearTextSelection() {
  window.getSelection()?.removeAllRanges();
}

interface Deps {
  files: FlatFile[];
  lines: DisplayLine[];
  markers: SymbolMarkers[];
  s: SelectionApi;
  rootRef: RefObject<HTMLDivElement | null>;
  setMenu(at: { x: number; y: number }): void;
  toggleHunk(file: number, hunk: number): void;
  toggleFile(file: number): void;
  setChecked(file: number, checked: boolean): void;
  reveal(gap: string): void;
}

/** Mouse handlers for the diff's lines: click, drag, and multi-click to select. */
export function useRowMouse(d: Deps): LineHandlers {
  const { files, lines, markers, s, rootRef } = d;
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
      // Only code text is selectable. Gutters and double/triple clicks pick rows, not text.
      const inCode = e.target instanceof Element && e.target.closest(".code");
      if (!inCode || e.detail >= 2) {
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
      if (root) d.setMenu({ x: e.clientX - root.left, y: e.clientY - root.top });
    },
    onFileClick(file: number) {
      const f = fileSelection(files, lines, file);
      select(f, f.start);
    },
    onHunkClick(file: number, hunk: number) {
      const h = hunkSelection(lines, file, hunk);
      select(h, h.start);
    },
    onToggleHunk: d.toggleHunk,
    onToggleFile: d.toggleFile,
    onCheckFile: d.setChecked,
    onGapClick(file: number, hunk: number, firstRow: number) {
      d.reveal(gapKey(file, hunk, firstRow));
    },
    onSymbolClick(i: number, file: number, row: number) {
      const marker = markers[file].get(row);
      if (!marker) return;
      const range = symbolRange(files, lines, file, marker.symbol, marker.side);
      if (range) {
        const label = `${marker.symbol.kind}: ${marker.symbol.name}`;
        select({ start: range[0], end: range[1], level: "function", label }, i);
      }
    },
  };
}
