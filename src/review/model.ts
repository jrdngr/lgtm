import type { CodeSymbol, ContextLine, FileDto, ReviewDto, RowDto } from "../bindings";
import type { DoneFn } from "./marks";
import { type Filter, type RowStateFn, rowMatches } from "./state";

export type ViewMode = "unified" | "split";

/** One line of the rendered review. */
export type DisplayLine =
  /** `folded`: the rows a collapse hides that would otherwise show. */
  | { kind: "file"; file: number; folded?: number[] }
  | { kind: "hunk"; file: number; hunk: number; folded?: number[] }
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
  | { kind: "note"; file: number; hunk: number; note: number }
  /** Unchanged code before hunk `hunk` (or after the last one), hidden behind a bar. */
  | { kind: "more"; file: number; hunk: number; region: Region }
  /** A line of that code once shown. `old` is its line on the old side. */
  | { kind: "context"; file: number; hunk: number; line: ContextLine; old: number };

/** New-side lines `from` to `to` that no hunk shows. `shift` takes a line to the old side. */
export interface Region {
  from: number;
  to: number;
  shift: number;
}

export const regionKey = (path: string, r: Region) => `${path}:${r.from}-${r.to}`;

/** The unchanged code before each hunk and after the last, by the hunk it precedes. */
export function regions(dto: FileDto): Map<number, Region> {
  const out = new Map<number, Region>();
  let next = 1;
  let shift = 0;
  dto.hunks.forEach((h, i) => {
    if (h.newStart > next)
      out.set(i, { from: next, to: h.newStart - 1, shift: h.oldStart - h.newStart });
    next = h.newStart + h.newLines;
    shift = h.oldStart + h.oldLines - next;
  });
  if (dto.hunks.length && dto.lineCount >= next)
    out.set(dto.hunks.length, { from: next, to: dto.lineCount, shift });
  return out;
}

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
  done: DoneFn;
  /** Gap bars clicked open, by `gapKey`. */
  revealed: ReadonlySet<string>;
  /** Comment ids by the `markKey` of the row their block follows, or `fileSlot` for ones
   * whose code is gone. */
  comments?: ReadonlyMap<string, number[]>;
  /** Walkthrough note ids, keyed the same way. */
  notes?: ReadonlyMap<string, number[]>;
  /** Unchanged code clicked open, by `regionKey`. */
  context?: ReadonlyMap<string, ContextLine[]>;
}

export function buildLines(files: FlatFile[], layout: Layout): DisplayLine[] {
  const { mode, filter, rowState, done } = layout;
  const out: DisplayLine[] = [];
  files.forEach((f, file) => {
    const matches = (r: number) =>
      needsReview(f.rows[r]) && rowMatches(rowState(file, r), filter, done);
    // A change's removed rows show with its added rows.
    const shown = (r: number) => filter === "all" || matches(r) || addedMatch(f.rows, r, matches);
    const shownIn = (from: number, to: number) =>
      Array.from({ length: to - from }, (_, i) => from + i).filter(shown);
    if (filter !== "all" && !f.rows.some((_, r) => shown(r))) return;
    if (layout.collapsedFiles.has(f.dto.path)) {
      out.push({ kind: "file", file, folded: shownIn(0, f.rows.length) });
      return;
    }
    out.push({ kind: "file", file });
    for (const comment of layout.comments?.get(fileSlot(file)) ?? [])
      out.push({ kind: "comment", file, hunk: -1, comment });
    for (const note of layout.notes?.get(fileSlot(file)) ?? [])
      out.push({ kind: "note", file, hunk: -1, note });
    const hidden = filter === "all" ? regions(f.dto) : new Map<number, Region>();
    const more = (hunk: number) => {
      const region = hidden.get(hunk);
      if (!region) return;
      const lines = layout.context?.get(regionKey(f.dto.path, region));
      if (!lines) return out.push({ kind: "more", file, hunk, region });
      for (const line of lines)
        out.push({ kind: "context", file, hunk, line, old: line.line + region.shift });
    };
    let start = 0;
    f.dto.hunks.forEach((h, hunk) => {
      more(hunk);
      const end = start + h.rows.length;
      const hunkStart = start;
      start = end;
      let any = false;
      for (let r = hunkStart; r < end && !any; r++) any = shown(r);
      if (!any && filter !== "all") return;
      if (layout.collapsedHunks.has(hunkKey(f.dto.path, hunk))) {
        out.push({ kind: "hunk", file, hunk, folded: shownIn(hunkStart, end) });
        return;
      }
      out.push({ kind: "hunk", file, hunk });
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
    more(f.dto.hunks.length);
  });
  return out;
}

/** Added rows, and removed rows of changes that add nothing. A change's removed rows
 * don't need review when it also adds rows. */
export const needsReview = (row: RowDto) => row.kind !== "context" && !row.replaced;

/** For a replaced row, whether an added row of its change matches. */
function addedMatch(rows: RowDto[], r: number, matches: (r: number) => boolean) {
  if (!rows[r].replaced) return false;
  let i = r;
  while (rows[i]?.kind === "removed") i++;
  for (; rows[i]?.kind === "added"; i++) if (matches(i)) return true;
  return false;
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

/** Every shown change's jump target, in display order: the first row of each run of changed
 * rows, so a long hunk with several edits stops at each. A hunk with no rows shown stops on
 * its header. A hunk's first stop keeps two lines above the header in view, so the sticky
 * file header can't cover it; later stops keep a few lines of context above. */
export function changeTargets(files: FlatFile[], lines: DisplayLine[]): JumpTarget[] {
  const out: JumpTarget[] = [];
  let header = 0;
  let first = true;
  let inChange = false;
  lines.forEach((l, i) => {
    if (l.kind === "hunk") {
      header = i;
      first = true;
      inChange = false;
      const next = lines[i + 1]?.kind;
      if (next !== "rows" && next !== "gap") out.push({ cursor: i, top: Math.max(i - 2, 0) });
    } else if (l.kind === "file" || l.kind === "gap") {
      inChange = false;
    } else if (l.kind === "rows") {
      const changed = l.rows.some((r) => files[l.file].rows[r].kind !== "context");
      if (changed && !inChange) {
        out.push({ cursor: i, top: first ? Math.max(header - 2, 0) : Math.max(i - 3, 0) });
        first = false;
      }
      inChange = changed;
    }
  });
  return out;
}

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

/** Changed rows on display lines `start..=end`, in display order. `folded` also takes the rows
 * collapsed under a file or hunk header. */
export function changedRows(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
  folded = false,
): Addr[] {
  const out: Addr[] = [];
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    const rows =
      line?.kind === "rows" ? line.rows : folded && line && "folded" in line ? line.folded : [];
    for (const row of rows ?? []) {
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
