import type { CodeSymbol, FileDto, MarkDto, ReviewDto, RowDto } from "../bindings";

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
    };

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

export const hunkKey = (file: number, hunk: number) => `${file}:${hunk}`;

export function buildLines(
  files: FlatFile[],
  mode: ViewMode,
  collapsed: ReadonlySet<string>,
): DisplayLine[] {
  const out: DisplayLine[] = [];
  files.forEach((f, file) => {
    out.push({ kind: "file", file });
    let start = 0;
    f.dto.hunks.forEach((h, hunk) => {
      out.push({ kind: "hunk", file, hunk });
      const end = start + h.rows.length;
      if (!collapsed.has(hunkKey(file, hunk))) {
        if (mode === "unified") {
          for (let r = start; r < end; r++) out.push({ kind: "rows", file, hunk, rows: [r] });
        } else {
          out.push(...splitRows(f.rows, start, end, file, hunk));
        }
      }
      start = end;
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

export function markMap(marks: MarkDto[]): Map<string, MarkDto> {
  return new Map(marks.map((m) => [markKey(m.file, m.row), m]));
}

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
    if (next.kind !== "rows" || next.file !== file || next.hunk !== hunk) break;
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

/** Reviewed and total changed rows per file. */
export function fileProgress(
  files: FlatFile[],
  marks: MarkDto[],
): { reviewed: number; changed: number }[] {
  const out = files.map((f) => ({
    reviewed: 0,
    changed: f.rows.filter((r) => r.kind !== "context").length,
  }));
  for (const m of marks) if (m.state === "reviewed" && out[m.file]) out[m.file].reviewed++;
  return out;
}
