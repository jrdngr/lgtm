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
  | { kind: "comment"; file: number; hunk: number; comment: number };

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

export function symbolsOf(file: FileDto, side: "old" | "new"): CodeSymbol[] {
  return side === "old" ? file.oldSymbols : file.newSymbols;
}

/** Symbols around `row` with a clean parse, innermost first. */
export function enclosingSymbols(file: FlatFile, row: number): CodeSymbol[] {
  const at = symbolSide(file.rows[row]);
  if (!at) return [];
  return symbolsOf(file.dto, at.side)
    .filter((s) => !s.hasError && s.startLine <= at.line && at.line <= s.endLine)
    .sort((a, b) => a.endLine - a.startLine - (b.endLine - b.startLine));
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
    const syms = symbolsOf(file.dto, at.side)
      .filter((s) => s.startLine <= at.line && at.line <= s.endLine)
      .sort((a, b) => a.endLine - a.startLine - (b.endLine - b.startLine));
    const fresh = syms.filter((s) => !seen.has(s));
    for (const s of fresh) seen.add(s);
    if (fresh.length > 0 && !out.has(r)) out.set(r, { symbol: fresh[0], side: at.side });
  });
  return out;
}
