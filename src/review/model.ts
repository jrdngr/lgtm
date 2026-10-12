import type { CodeSymbol, FileDto, ReviewDto, RowDto } from "../bindings";
import type { Filter, RowStateFn } from "./state";

export type ViewMode = "unified" | "split";

/** One line of the rendered review. */
export type DisplayLine =
  | { kind: "file"; file: number }
  | { kind: "hunk"; file: number; hunk: number }
  | {
      kind: "rows";
      file: number;
      hunk: number;
      /** File-flat row indexes shown on this line, in diff order. */
      rows: number[];
      /** Split view: the row on each side. A context row sits on both. */
      left?: number;
      right?: number;
    }
  /** A run of rows the line filter hides. */
  | { kind: "gap"; file: number; hunk: number; rows: number[] }
  /** A comment block, after the line holding its last row, or under the file header when its
   * code is gone (hunk -1). `NEW_COMMENT` is the draft. */
  | { kind: "comment"; file: number; hunk: number; comment: number }
  /** A walkthrough note's block, placed like a comment's. */
  | { kind: "note"; file: number; hunk: number; note: number };

/** The id of a comment being written that isn't saved yet. */
export const NEW_COMMENT = 0;

export type RowLine = Extract<DisplayLine, { kind: "rows" }>;

export interface FlatFile {
  dto: FileDto;
  rows: RowDto[];
  /** Hunk index per flat row. */
  rowHunk: number[];
}

export function flatten(review: ReviewDto): FlatFile[] {
  return review.files.map((dto) => {
    const rows: RowDto[] = [];
    const rowHunk: number[] = [];
    dto.hunks.forEach((h, i) => {
      for (const r of h.rows) {
        rows.push(r);
        rowHunk.push(i);
      }
    });
    return { dto, rows, rowHunk };
  });
}

/** Collapse keys use paths so they survive the diff changing under them. */
export const hunkKey = (path: string, hunk: number) => `${path}#${hunk}`;
export const gapKey = (file: number, hunk: number, firstRow: number) =>
  `${file}:${hunk}:${firstRow}`;

export interface Layout {
  mode: ViewMode;
  collapsedFiles: ReadonlySet<string>;
  collapsedHunks: ReadonlySet<string>;
  filter: Filter;
  rowState: RowStateFn;
  /** Gap bars clicked open, by `gapKey`. */
  revealed: ReadonlySet<string>;
  /** Comment ids by the `markKey` of the row their block follows, or `fileSlot` for ones
   * whose code is gone. */
  comments?: ReadonlyMap<string, number[]>;
  /** Walkthrough note ids, keyed the same way. */
  notes?: ReadonlyMap<string, number[]>;
}

export function buildLines(files: FlatFile[], layout: Layout): DisplayLine[] {
  const { mode, filter, rowState } = layout;
  const out: DisplayLine[] = [];
  files.forEach((f, file) => {
    const shown = (r: number) =>
      filter === "all" || (f.rows[r].kind !== "context" && rowState(file, r) === filter);
    if (filter !== "all" && !f.rows.some((_, r) => shown(r))) return;
    out.push({ kind: "file", file });
    if (layout.collapsedFiles.has(f.dto.path)) return;
    for (const comment of layout.comments?.get(fileSlot(file)) ?? [])
      out.push({ kind: "comment", file, hunk: -1, comment });
    for (const note of layout.notes?.get(fileSlot(file)) ?? [])
      out.push({ kind: "note", file, hunk: -1, note });
    let start = 0;
    f.dto.hunks.forEach((h, hunk) => {
      const end = start + h.rows.length;
      const hunkStart = start;
      start = end;
      let any = false;
      for (let r = hunkStart; r < end && !any; r++) any = shown(r);
      if (!any && filter !== "all") return;
      out.push({ kind: "hunk", file, hunk });
      if (layout.collapsedHunks.has(hunkKey(f.dto.path, hunk))) return;
      // Alternate runs of shown rows and gaps.
      let r = hunkStart;
      while (r < end) {
        const runStart = r;
        const visible = shown(r) || layout.revealed.has(gapKey(file, hunk, r));
        if (visible) {
          while (r < end && shown(r)) r++;
          if (r === runStart) {
            // A revealed gap: show its rows until the next shown row.
            while (r < end && !shown(r)) r++;
          }
          const rowLines: RowLine[] =
            mode === "unified"
              ? Array.from({ length: r - runStart }, (_, i) => ({
                  kind: "rows",
                  file,
                  hunk,
                  rows: [runStart + i],
                }))
              : splitRows(f.rows, runStart, r, file, hunk);
          for (const line of rowLines) {
            out.push(line);
            for (const row of line.rows) {
              for (const comment of layout.comments?.get(markKey(file, row)) ?? [])
                out.push({ kind: "comment", file, hunk, comment });
              for (const note of layout.notes?.get(markKey(file, row)) ?? [])
                out.push({ kind: "note", file, hunk, note });
            }
          }
        } else {
          while (r < end && !shown(r)) r++;
          const rows = Array.from({ length: r - runStart }, (_, i) => runStart + i);
          out.push({ kind: "gap", file, hunk, rows });
        }
      }
    });
  });
  return out;
}

/** Pairs each run of removed rows with the added rows that follow it, side by side. */
function splitRows(rows: RowDto[], start: number, end: number, file: number, hunk: number) {
  const out: RowLine[] = [];
  let r = start;
  while (r < end) {
    if (rows[r].kind === "context") {
      out.push({ kind: "rows", file, hunk, rows: [r], left: r, right: r });
      r++;
      continue;
    }
    const removed: number[] = [];
    const added: number[] = [];
    while (r < end && rows[r].kind === "removed") removed.push(r++);
    while (r < end && rows[r].kind === "added") added.push(r++);
    for (let i = 0; i < Math.max(removed.length, added.length); i++) {
      const left = removed[i];
      const right = added[i];
      out.push({
        kind: "rows",
        file,
        hunk,
        rows: [left, right].filter((x) => x !== undefined),
        left,
        right,
      });
    }
  }
  return out;
}

export const SIGN = { context: " ", added: "+", removed: "-" } as const;

/** The first rows line from `from` in direction `dir`, or `from` when there is none. */
export function nextRowLine(lines: DisplayLine[], from: number, dir: 1 | -1): number {
  for (let i = from + dir; i >= 0 && i < lines.length; i += dir) {
    if (lines[i].kind === "rows") return i;
  }
  return from;
}

export interface JumpTarget {
  cursor: number;
  /** The line to scroll to the top. */
  top: number;
}

/** Jumping to a row puts the cursor on it with a few lines of context above. */
export const rowJumpTarget = (at: number): JumpTarget => ({ cursor: at, top: Math.max(at - 3, 0) });

/** Jumping to a file scrolls its header to the top and puts the cursor on its first row. */
export function fileJumpTarget(lines: DisplayLine[], file: number): JumpTarget | undefined {
  const header = lines.findIndex((l) => l.kind === "file" && l.file === file);
  return header < 0 ? undefined : fileTargetAt(lines, header);
}

function fileTargetAt(lines: DisplayLine[], header: number): JumpTarget {
  const cursor = lines[header + 1]?.kind === "file" ? header : nextRowLine(lines, header, 1);
  return { cursor, top: header };
}

/** Every shown file's jump target, in display order. */
export const fileTargets = (lines: DisplayLine[]): JumpTarget[] =>
  lines.flatMap((l, i) => (l.kind === "file" ? [fileTargetAt(lines, i)] : []));

/** Jumping to a hunk scrolls its header to the top and puts the cursor on its first row, or on
 * the header when no row of it is shown. */
export function hunkJumpTarget(lines: DisplayLine[], header: number): JumpTarget {
  for (let i = header + 1; i < lines.length; i++) {
    const kind = lines[i].kind;
    if (kind === "rows") return { cursor: i, top: header };
    if (kind === "file" || kind === "hunk") break;
  }
  return { cursor: header, top: header };
}

/** Every shown hunk's jump target, in display order. */
export const hunkTargets = (lines: DisplayLine[]): JumpTarget[] =>
  lines.flatMap((l, i) => (l.kind === "hunk" ? [hunkJumpTarget(lines, i)] : []));

/** The first target after `cursor` (`dir` 1) or the last one before it (-1). Doesn't wrap. */
export function stepTarget(
  targets: JumpTarget[],
  cursor: number,
  dir: 1 | -1,
): JumpTarget | undefined {
  if (dir === 1) return targets.find((t) => t.cursor > cursor);
  return [...targets].reverse().find((t) => t.cursor < cursor);
}

/** Jumping to a comment block puts the cursor on the row it follows. */
export function commentJumpTarget(lines: DisplayLine[], id: number): JumpTarget | undefined {
  const at = lines.findIndex((l) => l.kind === "comment" && l.comment === id);
  if (at < 0) return undefined;
  // A block under a file header has no row above it in its file.
  const line = lines[at];
  const cursor = nextRowLine(lines, at, line.kind === "comment" && line.hunk === -1 ? 1 : -1);
  return { cursor, top: Math.max(at - 3, 0) };
}

export interface Addr {
  file: number;
  row: number;
}

/** Changed rows on display lines `start..=end`, in display order. */
export function changedRows(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): Addr[] {
  const out: Addr[] = [];
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind !== "rows") continue;
    for (const row of line.rows) {
      if (files[line.file].rows[row].kind !== "context") out.push({ file: line.file, row });
    }
  }
  return out;
}

export const markKey = (file: number, row: number) => `${file}:${row}`;
/** Where comments whose code is gone sit: under their file's header. */
export const fileSlot = (file: number) => `${file}:file`;

/** The side whose line numbers place `row` within that side's symbols. */
export function symbolSide(row: RowDto): { line: number; side: "old" | "new" } | undefined {
  if (row.kind === "removed") return row.oldLine ? { line: row.oldLine, side: "old" } : undefined;
  return row.newLine ? { line: row.newLine, side: "new" } : undefined;
}

/** Symbols on `side` containing `line`, innermost first. */
function symbolsAt(dto: FileDto, side: "old" | "new", line: number): CodeSymbol[] {
  return (side === "old" ? dto.oldSymbols : dto.newSymbols)
    .filter((s) => s.startLine <= line && line <= s.endLine)
    .sort((a, b) => a.endLine - a.startLine - (b.endLine - b.startLine));
}

/** Symbols around `row` with a clean parse, innermost first. */
export function enclosingSymbols(file: FlatFile, row: number): CodeSymbol[] {
  const at = symbolSide(file.rows[row]);
  if (!at) return [];
  return symbolsAt(file.dto, at.side, at.line).filter((s) => !s.hasError);
}

/** Display range of the rows inside `symbol` on `side`, or undefined if none are shown. */
export function symbolRange(
  files: FlatFile[],
  lines: DisplayLine[],
  file: number,
  symbol: CodeSymbol,
  side: "old" | "new",
): [number, number] | undefined {
  let first: number | undefined;
  let last: number | undefined;
  lines.forEach((line, i) => {
    if (line.kind !== "rows" || line.file !== file) return;
    const inside = line.rows.some((r) => {
      const row = files[file].rows[r];
      const n = side === "old" ? row.oldLine : row.newLine;
      return n !== null && symbol.startLine <= n && n <= symbol.endLine;
    });
    if (inside) {
      first ??= i;
      last = i;
    }
  });
  return first === undefined || last === undefined ? undefined : [first, last];
}

export function hunkRange(lines: DisplayLine[], file: number, hunk: number): [number, number] {
  const start = lines.findIndex((l) => l.kind === "hunk" && l.file === file && l.hunk === hunk);
  let end = start;
  while (end + 1 < lines.length) {
    const next = lines[end + 1];
    if (next.kind === "file" || next.kind === "hunk" || next.file !== file || next.hunk !== hunk)
      break;
    end++;
  }
  return [start, end];
}

export function fileRange(lines: DisplayLine[], file: number): [number, number] {
  const start = lines.findIndex((l) => l.kind === "file" && l.file === file);
  let end = start;
  while (end + 1 < lines.length && lines[end + 1].kind !== "file") end++;
  return [start, end];
}

/** Flat row → the symbol whose ƒ marker sits on that row. */
export type SymbolMarkers = Map<number, { symbol: CodeSymbol; side: "old" | "new" }>;

/** ƒ markers: the first changed row of each changed symbol gets the innermost symbol. */
export function symbolMarkers(file: FlatFile): SymbolMarkers {
  const out: SymbolMarkers = new Map();
  const seen = new Set<CodeSymbol>();
  file.rows.forEach((row, r) => {
    if (row.kind === "context") return;
    const at = symbolSide(row);
    if (!at) return;
    const fresh = symbolsAt(file.dto, at.side, at.line).filter((s) => !seen.has(s));
    for (const s of fresh) seen.add(s);
    if (fresh.length > 0) out.set(r, { symbol: fresh[0], side: at.side });
  });
  return out;
}
