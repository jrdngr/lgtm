import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, type RefObject, useCallback, useEffect, useMemo } from "react";
import { wordWrap } from "../workbench/wordWrap";
import { Cell } from "./Cell";
import { FileHeader } from "./FileHeader";
import type { MarkSetup } from "./marks";
import {
  type DisplayLine,
  type FlatFile,
  hunkKey,
  type SymbolMarkers,
  type ViewMode,
} from "./model";
import { SplitScroll } from "./SplitScroll";
import type { Selection } from "./selection";
import { type MarkIndex, type Progress, plural } from "./state";
import { unifiedWidth } from "./width";

const ROW_HEIGHT = 20;
const FILE_HEIGHT = 30;
const HEIGHTS: Record<DisplayLine["kind"], number> = {
  file: FILE_HEIGHT,
  hunk: ROW_HEIGHT,
  rows: ROW_HEIGHT,
  gap: 8,
  comment: 120,
  note: 80,
};

function lineKey(line: DisplayLine): string {
  switch (line.kind) {
    case "file":
      return `f:${line.file}`;
    case "hunk":
      return `h:${line.file}:${line.hunk}`;
    case "rows":
      return `r:${line.file}:${line.rows.join(",")}`;
    case "gap":
      return `g:${line.file}:${line.rows[0]}`;
    case "comment":
      return `c:${line.comment}`;
    case "note":
      return `n:${line.note}`;
  }
}

export interface LineHandlers {
  onRowMouseDown(e: React.MouseEvent, line: number): void;
  onRowMouseEnter(line: number): void;
  onRowContextMenu(e: React.MouseEvent, line: number): void;
  onFileClick(file: number): void;
  onHunkClick(file: number, hunk: number): void;
  onToggleHunk(file: number, hunk: number): void;
  onToggleFile(file: number): void;
  onCheckFile(file: number, checked: boolean): void;
  onGapClick(file: number, hunk: number, firstRow: number): void;
  onSymbolClick(line: number, file: number, row: number): void;
}

interface Props extends LineHandlers {
  files: FlatFile[];
  markers: SymbolMarkers[];
  lines: DisplayLine[];
  mode: ViewMode;
  index: MarkIndex;
  setup: MarkSetup;
  progress: Progress[];
  selection: Selection;
  cursor: number;
  collapsedFiles: ReadonlySet<string>;
  collapsedHunks: ReadonlySet<string>;
  scrollRef: RefObject<HTMLDivElement | null>;
  /** Scrolls this line to the top whenever a new object is passed. */
  scrollTo?: { line: number };
  onBackgroundMouseDown(): void;
  onClearFile(file: number): void;
  renderComment(id: number): ReactNode;
  renderNote(id: number): ReactNode;
}

/** The header of the file at the top of the viewport, once its own header has scrolled
 * away. The next file's header pushes it up as it arrives. */
function stickyFile(
  lines: DisplayLine[],
  v: ReturnType<typeof useVirtualizer<HTMLDivElement, Element>>,
): { file: number; shift: number } | undefined {
  const offset = v.scrollOffset ?? 0;
  const top = v.getVirtualItems().find((item) => item.end > offset);
  if (!top) return undefined;
  const file = lines[top.index]?.file;
  if (file === undefined) return undefined;
  let headerIdx = top.index;
  while (headerIdx > 0 && lines[headerIdx].kind !== "file") headerIdx--;
  const measured = v.measurementsCache;
  if ((measured[headerIdx]?.start ?? 0) >= offset) return undefined;
  let next = top.index + 1;
  while (next < lines.length && lines[next].kind !== "file") next++;
  const nextStart = measured[next]?.start;
  const gap = nextStart === undefined ? FILE_HEIGHT : nextStart - offset;
  return { file, shift: Math.min(0, gap - FILE_HEIGHT) };
}

export function DiffLines(props: Props) {
  const { files, markers, lines, mode, index, selection, cursor, scrollRef, scrollTo } = props;
  const wrap = wordWrap.use();
  // Split sides scroll sideways on their own, under `SplitScroll`.
  const splitScroll = mode === "split" && !wrap;
  const width = useMemo(
    () => (wrap || mode === "split" ? undefined : unifiedWidth(files)),
    [files, mode, wrap],
  );

  const virtualizer = useVirtualizer({
    count: lines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => HEIGHTS[lines[i].kind],
    // Keyed by content, so a measured comment's height stays with it as lines shift. A new
    // function per `lines` also makes the virtualizer re-estimate when lines change.
    getItemKey: useCallback((i: number) => lineKey(lines[i]), [lines]),
    overscan: 30,
  });

  // Wrapped rows take their measured heights; unwrapped ones go back to fixed heights.
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-measure when wrapping changes
  useEffect(() => {
    virtualizer.measure();
  }, [wrap, virtualizer]);

  // Lines that don't scroll sideways (headers, comments) fit the visible width.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const fit = () => el.style.setProperty("--view-w", `${el.clientWidth}px`);
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => observer.disconnect();
  }, [scrollRef]);

  useEffect(() => {
    virtualizer.scrollToIndex(cursor, { align: "auto" });
  }, [cursor, virtualizer]);

  // After the cursor effect, so a jump wins over its scroll.
  useEffect(() => {
    if (scrollTo) virtualizer.scrollToIndex(scrollTo.line, { align: "start" });
  }, [scrollTo, virtualizer]);

  const sticky = stickyFile(lines, virtualizer);

  const header = (fileIdx: number) => {
    const file = files[fileIdx];
    return (
      <FileHeader
        dto={file.dto}
        check={index.checks[fileIdx] ?? "unchecked"}
        progress={props.progress[fileIdx]}
        setup={props.setup}
        collapsed={props.collapsedFiles.has(file.dto.path)}
        onClick={() => props.onFileClick(fileIdx)}
        onToggle={() => props.onToggleFile(fileIdx)}
        onCheck={(checked) => props.onCheckFile(fileIdx, checked)}
        onClear={() => props.onClearFile(fileIdx)}
      />
    );
  };

  const render = (line: DisplayLine, i: number) => {
    const file = files[line.file];
    if (line.kind === "file") return header(line.file);
    if (line.kind === "comment") return props.renderComment(line.comment);
    if (line.kind === "note") return props.renderNote(line.note);
    if (line.kind === "gap") {
      const n = line.rows.length;
      return (
        <div
          className="gap-bar"
          title="Show hidden rows"
          onMouseDown={(e) => {
            e.stopPropagation();
            props.onGapClick(line.file, line.hunk, line.rows[0]);
          }}
        >
          {n} hidden {plural(n)}
        </div>
      );
    }
    if (line.kind === "hunk") {
      const h = file.dto.hunks[line.hunk];
      const isCollapsed = props.collapsedHunks.has(hunkKey(file.dto.path, line.hunk));
      return (
        <div className="hunk-header" onMouseDown={() => props.onHunkClick(line.file, line.hunk)}>
          <button
            type="button"
            className="collapse"
            onMouseDown={(e) => {
              e.stopPropagation();
              props.onToggleHunk(line.file, line.hunk);
            }}
          >
            {isCollapsed ? "▸" : "▾"}
          </button>
          @@ -{h.oldStart},{h.oldLines} +{h.newStart},{h.newLines} @@ {h.section}
        </div>
      );
    }
    const markerFor = (row: number | undefined) => {
      const m = row === undefined ? undefined : markers[line.file].get(row);
      return (
        m && {
          name: m.symbol.name,
          onClick: () => props.onSymbolClick(i, line.file, row as number),
        }
      );
    };
    const cells =
      mode === "unified" ? (
        <Cell
          file={file}
          fileIdx={line.file}
          row={line.rows[0]}
          index={index}
          setup={props.setup}
          marker={markerFor(line.rows[0])}
        />
      ) : (
        <>
          <Cell
            file={file}
            fileIdx={line.file}
            row={line.left}
            index={index}
            setup={props.setup}
            side="left"
            marker={markerFor(line.left)}
          />
          <Cell
            file={file}
            fileIdx={line.file}
            row={line.right}
            index={index}
            setup={props.setup}
            side="right"
            marker={line.right !== line.left ? markerFor(line.right) : undefined}
          />
        </>
      );
    return (
      <div
        className={`row-line ${mode}`}
        onMouseDown={(e) => props.onRowMouseDown(e, i)}
        onMouseEnter={() => props.onRowMouseEnter(i)}
        onContextMenu={(e) => props.onRowContextMenu(e, i)}
      >
        {cells}
      </div>
    );
  };

  return (
    <>
      <div
        ref={scrollRef}
        className={`diff-scroll${wrap ? " wrap" : ""}${splitScroll ? " split" : ""}`}
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) props.onBackgroundMouseDown();
        }}
      >
        {sticky && (
          <div className="sticky-file">
            <div
              className="line"
              style={{ height: FILE_HEIGHT, transform: `translateY(${sticky.shift}px)` }}
            >
              {header(sticky.file)}
            </div>
          </div>
        )}
        <div style={{ height: virtualizer.getTotalSize(), width, position: "relative" }}>
          {virtualizer.getVirtualItems().map((v) => {
            const selected = v.index >= selection.start && v.index <= selection.end;
            // Comment and note blocks size to their content, as do rows when they wrap. Every
            // other line has a fixed height.
            const kind = lines[v.index].kind;
            const sized = kind === "comment" || kind === "note" || (wrap && kind === "rows");
            return (
              <div
                key={v.key}
                ref={sized ? virtualizer.measureElement : undefined}
                data-index={v.index}
                className={`line${selected ? " selected" : ""}${v.index === cursor ? " cursor" : ""}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  right: 0,
                  height: sized ? undefined : v.size,
                  transform: `translateY(${v.start}px)`,
                }}
              >
                {kind === "rows" ? (
                  render(lines[v.index], v.index)
                ) : (
                  <div className="pin">{render(lines[v.index], v.index)}</div>
                )}
              </div>
            );
          })}
        </div>
      </div>
      {splitScroll && <SplitScroll files={files} scrollRef={scrollRef} />}
    </>
  );
}
