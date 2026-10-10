import { useVirtualizer } from "@tanstack/react-virtual";
import { type RefObject, useEffect } from "react";
import { HIGHLIGHT_NAMES, type MarkDto, type RowDto } from "../bindings";
import {
  type DisplayLine,
  type FlatFile,
  hunkKey,
  markKey,
  type SymbolMarkers,
  type ViewMode,
} from "./model";
import type { Selection } from "./selection";

const ROW_HEIGHT = 20;
const FILE_HEIGHT = 30;

const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

function Code({ row }: { row: RowDto }) {
  if (!row.hl) return <span className="code">{row.text}</span>;
  let pos = 0;
  return (
    <span className="code">
      {row.hl.map(([cls, len]) => {
        const start = pos;
        pos += len;
        const piece = row.text.slice(start, pos);
        return cls === null ? (
          piece
        ) : (
          <span key={start} className={hlClass[cls]}>
            {piece}
          </span>
        );
      })}
      {row.text.slice(pos)}
    </span>
  );
}

function markTitle(m: MarkDto | undefined) {
  if (!m) return undefined;
  const when = m.createdAt === null ? "" : ` · ${new Date(m.createdAt).toLocaleString()}`;
  return `${m.state} in ${m.review}${when}`;
}

const SIGN = { context: " ", added: "+", removed: "-" } as const;

interface CellProps {
  file: FlatFile;
  fileIdx: number;
  row: number | undefined;
  marks: Map<string, MarkDto>;
  marker?: { name: string; onClick: () => void };
  side?: "left" | "right";
}

function Cell({ file, fileIdx, row, marks, marker, side }: CellProps) {
  if (row === undefined) return <div className="cell empty" />;
  const r = file.rows[row];
  const mark = r.kind === "context" ? undefined : marks.get(markKey(fileIdx, row));
  const lineNo = side === "left" ? r.oldLine : side === "right" ? r.newLine : undefined;
  return (
    <div
      className={`cell kind-${r.kind}${mark ? ` mark-${mark.state}` : ""}`}
      title={markTitle(mark)}
    >
      <span className="strip" />
      <span
        className={`fn-marker${marker ? " on" : ""}`}
        title={marker ? `select ${marker.name}` : undefined}
        onMouseDown={(e) => {
          if (!marker) return;
          e.stopPropagation();
          marker.onClick();
        }}
      >
        {marker ? "ƒ" : ""}
      </span>
      {side ? (
        <span className="ln">{lineNo ?? ""}</span>
      ) : (
        <>
          <span className="ln">{r.oldLine ?? ""}</span>
          <span className="ln">{r.newLine ?? ""}</span>
        </>
      )}
      <span className="sign">{SIGN[r.kind]}</span>
      <Code row={r} />
    </div>
  );
}

export interface LineHandlers {
  onRowMouseDown(e: React.MouseEvent, line: number): void;
  onRowMouseEnter(line: number): void;
  onRowContextMenu(e: React.MouseEvent, line: number): void;
  onFileClick(file: number): void;
  onHunkClick(file: number, hunk: number): void;
  onToggleHunk(file: number, hunk: number): void;
  onSymbolClick(line: number, file: number, row: number): void;
}

interface Props extends LineHandlers {
  files: FlatFile[];
  markers: SymbolMarkers[];
  lines: DisplayLine[];
  mode: ViewMode;
  marks: Map<string, MarkDto>;
  selection: Selection;
  cursor: number;
  collapsed: ReadonlySet<string>;
  scrollRef: RefObject<HTMLDivElement | null>;
  /** Scrolls this line to the top whenever a new object is passed. */
  scrollTo?: { line: number };
  onBackgroundMouseDown(): void;
}

export const STATUS_LABEL = {
  modified: "M",
  added: "A",
  removed: "D",
  copied: "C",
  renamed: "R",
} as const;

export function DiffLines(props: Props) {
  const { files, markers, lines, mode, marks, selection, cursor, collapsed, scrollRef, scrollTo } =
    props;

  const virtualizer = useVirtualizer({
    count: lines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (lines[i].kind === "file" ? FILE_HEIGHT : ROW_HEIGHT),
    overscan: 30,
  });

  useEffect(() => {
    virtualizer.scrollToIndex(cursor, { align: "auto" });
  }, [cursor, virtualizer]);

  // After the cursor effect, so a jump wins over its scroll.
  useEffect(() => {
    if (scrollTo) virtualizer.scrollToIndex(scrollTo.line, { align: "start" });
  }, [scrollTo, virtualizer]);

  const render = (line: DisplayLine, i: number) => {
    const file = files[line.file];
    if (line.kind === "file") {
      const d = file.dto;
      return (
        <div className="file-header" onMouseDown={() => props.onFileClick(line.file)}>
          <span className={`status status-${d.status}`}>{STATUS_LABEL[d.status]}</span>
          <span className="path">{d.oldPath ? `${d.oldPath} → ${d.path}` : d.path}</span>
          {d.oldExec !== d.newExec && <span className="tag">{d.newExec ? "+x" : "-x"}</span>}
          {d.binary && <span className="tag">binary</span>}
          {d.conflict && <span className="tag conflict">conflict</span>}
        </div>
      );
    }
    if (line.kind === "hunk") {
      const h = file.dto.hunks[line.hunk];
      const isCollapsed = collapsed.has(hunkKey(line.file, line.hunk));
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
          marks={marks}
          marker={markerFor(line.rows[0])}
        />
      ) : (
        <>
          <Cell
            file={file}
            fileIdx={line.file}
            row={line.left}
            marks={marks}
            side="left"
            marker={markerFor(line.left)}
          />
          <Cell
            file={file}
            fileIdx={line.file}
            row={line.right}
            marks={marks}
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
    <div
      ref={scrollRef}
      className="diff-scroll"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) props.onBackgroundMouseDown();
      }}
    >
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((v) => {
          const selected = v.index >= selection.start && v.index <= selection.end;
          return (
            <div
              key={v.key}
              className={`line${selected ? " selected" : ""}${v.index === cursor ? " cursor" : ""}`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                right: 0,
                height: v.size,
                transform: `translateY(${v.start}px)`,
              }}
            >
              {render(lines[v.index], v.index)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
