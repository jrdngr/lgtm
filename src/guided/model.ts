import type {
  ChunkDto,
  ChunkRisk,
  CommentDto,
  ExcludedDto,
  ExcludeReason,
  GuidedDto,
  GuidedRange,
  VerdictDto,
} from "../bindings";
import { SIGN } from "../model/sign";
import type { LineStat } from "../review/lineStat";
import { type FlatFile, markKey } from "../review/model";

export const RISKS: ChunkRisk[] = ["high", "medium", "low"];

export const RISK_NAMES: Record<ChunkRisk, string> = { high: "High", medium: "Med", low: "Low" };

export const REASON_NAMES: Record<ExcludeReason, string> = {
  docs: "Docs",
  simple: "Simple impl",
  boilerplate: "Boilerplate",
  generated: "Generated",
  tests: "Tests",
  other: "Other",
  unlisted: "Unlisted",
};

/** A run of one file's rows, by index among its flat rows, that a chunk shows together. */
export interface Segment {
  file: number;
  path: string;
  rows: number[];
}

/** Each row's place on the new side: its line, or for a removed row the line after the last
 * new line before it. */
function positions(f: FlatFile): number[] {
  let last = 0;
  return f.rows.map((r) => {
    if (r.newLine === null) return last + 1;
    last = r.newLine;
    return r.newLine;
  });
}

/** The review rows `ranges` cover, as runs of adjacent rows in one hunk, files in the order
 * the ranges name them. Ranges on files the review doesn't change are left out. */
export function segmentsOf(files: FlatFile[], ranges: GuidedRange[]): Segment[] {
  const picked = new Map<number, Set<number>>();
  for (const range of ranges) {
    const file = files.findIndex((f) => f.dto.path === range.path);
    if (file < 0) continue;
    const set = picked.get(file) ?? new Set<number>();
    picked.set(file, set);
    positions(files[file]).forEach((at, row) => {
      if (at >= range.start && at <= range.end) set.add(row);
    });
  }
  const out: Segment[] = [];
  for (const [file, set] of picked) {
    const f = files[file];
    let run: number[] = [];
    const flush = () => {
      if (run.length) out.push({ file, path: f.dto.path, rows: run });
      run = [];
    };
    for (const row of [...set].sort((a, b) => a - b)) {
      const prev = run[run.length - 1];
      if (prev !== undefined && (row !== prev + 1 || f.rowHunk[row] !== f.rowHunk[prev])) flush();
      run.push(row);
    }
    flush();
  }
  return out;
}

/** The changed rows of `segments`. */
function changedRows(files: FlatFile[], segments: Segment[]) {
  return segments.flatMap((s) =>
    s.rows
      .filter((r) => files[s.file].rows[r].kind !== "context")
      .map((row) => ({ file: s.file, row })),
  );
}

/** How many changed lines `segments` hold. */
export const lineCount = (files: FlatFile[], segments: Segment[]) =>
  changedRows(files, segments).length;

/** How many lines `segments` add and remove. */
export function segmentStat(files: FlatFile[], segments: Segment[]): LineStat {
  const stat = { added: 0, removed: 0 };
  for (const { file, row } of changedRows(files, segments)) {
    if (files[file].rows[row].kind === "added") stat.added++;
    else stat.removed++;
  }
  return stat;
}

/** A chunk's code as diff text, per file under its path: what a verdict remembers, to show
 * what changed since. */
export function chunkText(files: FlatFile[], segments: Segment[]): string {
  return segments
    .map((s) =>
      [
        s.path,
        ...s.rows.map((r) => {
          const row = files[s.file].rows[r];
          return `${SIGN[row.kind]}${row.text}`;
        }),
      ].join("\n"),
    )
    .join("\n");
}

/** Where a chunk's lines are, one `path:start-end` per range, for an agent to read. */
export const rangesText = (ranges: GuidedRange[]) =>
  ranges.map((r) => `${r.path}:${r.start === r.end ? r.start : `${r.start}-${r.end}`}`).join("\n");

export interface ChunkState {
  chunk: ChunkDto;
  segments: Segment[];
  text: string;
  lines: number;
  stat: LineStat;
  verdict?: VerdictDto;
  /** Its code changed since its verdict, so the verdict no longer holds. */
  changed: boolean;
  /** Its verdict holds. */
  done: boolean;
}

/** Each chunk with its rows and where its verdict stands, High first, then Med, then Low. */
export function chunkStates(files: FlatFile[], g: GuidedDto): ChunkState[] {
  const verdicts = new Map(g.verdicts);
  const out = g.chunks.map((chunk) => {
    const segments = segmentsOf(files, chunk.ranges);
    const text = chunkText(files, segments);
    const verdict = verdicts.get(chunk.id);
    const changed = !!verdict && verdict.text !== text;
    return {
      chunk,
      segments,
      text,
      lines: lineCount(files, segments),
      stat: segmentStat(files, segments),
      verdict,
      changed,
      done: !!verdict && !changed && verdict.verdict !== "change",
    };
  });
  return RISKS.flatMap((risk) => out.filter((s) => s.chunk.risk === risk));
}

/** The chunk after `id` still to review, going round to the start; `undefined` when all are
 * done. */
export function nextChunk(states: ChunkState[], id: string): string | undefined {
  const at = states.findIndex((s) => s.chunk.id === id);
  for (let i = 1; i <= states.length; i++) {
    const s = states[(at + i) % states.length];
    if (s.chunk.id !== id && !s.done && s.verdict?.verdict !== "change") return s.chunk.id;
  }
  return undefined;
}

export interface Progress {
  /** Changed lines in chunks still to approve, by risk. */
  open: Record<ChunkRisk, number>;
  approved: number;
  skipped: number;
  total: number;
}

export function progressOf(states: ChunkState[]): Progress {
  const p: Progress = { open: { high: 0, medium: 0, low: 0 }, approved: 0, skipped: 0, total: 0 };
  for (const s of states) {
    p.total += s.lines;
    if (s.done && s.verdict?.verdict === "approved") p.approved += s.lines;
    else if (s.done && s.verdict?.verdict === "skipped") p.skipped += s.lines;
    else p.open[s.chunk.risk] += s.lines;
  }
  return p;
}

/** A PR thread or draft on the review's code. */
const isThread = (c: CommentDto) => !!(c.github || c.thread) && !c.thread?.quote && !!c.rows;

/** Whether comment `c`'s rows fall in `segments`. */
export function inSegments(c: CommentDto, segments: Segment[]): boolean {
  const span = c.rows;
  if (!span) return false;
  return segments.some(
    (s) => s.file === c.file && s.rows.some((r) => r >= span.start && r <= span.end),
  );
}

/** The PR threads on a chunk's code. */
export const threadsIn = (comments: CommentDto[], segments: Segment[]) =>
  comments.filter((c) => isThread(c) && inSegments(c, segments));

/** PR threads on code no chunk holds: they show anyway, as Commented. */
export const commentedThreads = (comments: CommentDto[], states: ChunkState[]) =>
  comments.filter((c) => isThread(c) && !states.some((s) => inSegments(c, s.segments)));

/** A thread's rows with a few around them, for showing a Commented thread's code. */
export function threadSegment(files: FlatFile[], c: CommentDto, around = 3): Segment[] {
  const f = files[c.file];
  if (!f || !c.rows) return [];
  const hunk = f.rowHunk[c.rows.start];
  const rows: number[] = [];
  for (let r = Math.max(0, c.rows.start - around); r <= c.rows.end + around; r++) {
    if (r < f.rows.length && f.rowHunk[r] === hunk) rows.push(r);
  }
  return [{ file: c.file, path: f.dto.path, rows }];
}

export type DiffLine = { kind: "same" | "added" | "removed"; text: string };

/** A line diff of `before` to `after`, by longest common subsequence. */
export function lineDiff(before: string, after: string): DiffLine[] {
  const a = before.split("\n");
  const b = after.split("\n");
  const n = a.length;
  const m = b.length;
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ kind: "same", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) out.push({ kind: "removed", text: a[i++] });
    else out.push({ kind: "added", text: b[j++] });
  }
  while (i < n) out.push({ kind: "removed", text: a[i++] });
  while (j < m) out.push({ kind: "added", text: b[j++] });
  return out;
}

/** Whether new-side `line` of `path` is in one of `ranges`. */
export const inRanges = (ranges: GuidedRange[], path: string, line: number | null) =>
  line !== null && ranges.some((r) => r.path === path && line >= r.start && line <= r.end);

/** A thread's first post, in a line. */
export const threadTitle = (c: CommentDto) =>
  (c.github?.posts[0]?.body ?? c.body).split("\n")[0].trim() || c.path;

export interface ExcludedState {
  part: ExcludedDto;
  /** Its place in the guided review's excluded list, which Promote names. */
  index: number;
  segments: Segment[];
  lines: number;
  stat: LineStat;
}

/** Everything a guided review shows over the review's files and comments. */
export interface GuidedView {
  states: ChunkState[];
  progress: Progress;
  commented: CommentDto[];
  /** Excluded test code, which has a group of its own. */
  tests: ExcludedState[];
  excluded: ExcludedState[];
}

export function guidedView(files: FlatFile[], comments: CommentDto[], g: GuidedDto): GuidedView {
  const states = chunkStates(files, g);
  const parts = g.excluded.map((part, index) => {
    const segments = segmentsOf(files, part.ranges);
    return {
      part,
      index,
      segments,
      lines: lineCount(files, segments),
      stat: segmentStat(files, segments),
    };
  });
  return {
    states,
    progress: progressOf(states),
    commented: commentedThreads(comments, states),
    tests: parts.filter((p) => p.part.reason === "tests"),
    excluded: parts.filter((p) => p.part.reason !== "tests"),
  };
}

/** The code a step shows: the runs of rows and related ranges its lines touch. A step
 * without lines, or whose lines touch none, shows everything. */
export function stepParts(
  files: FlatFile[],
  segments: Segment[],
  related: GuidedRange[],
  lit: GuidedRange[] | undefined,
): { segments: Segment[]; related: GuidedRange[] } {
  if (!lit?.length) return { segments, related };
  const litRows = new Set(
    segmentsOf(files, lit).flatMap((g) => g.rows.map((r) => markKey(g.file, r))),
  );
  const shown = segments.filter((g) => g.rows.some((r) => litRows.has(markKey(g.file, r))));
  const near = related.filter((r) =>
    lit.some((l) => l.path === r.path && l.start <= r.end && l.end >= r.start),
  );
  return shown.length || near.length ? { segments: shown, related: near } : { segments, related };
}

/** `segments` with each file in `expanded` shown whole: every hunk of it, at the place of its
 * first segment. */
export function withWholeFiles(
  files: FlatFile[],
  segments: Segment[],
  expanded: ReadonlySet<number>,
): Segment[] {
  const out: Segment[] = [];
  const done = new Set<number>();
  for (const seg of segments) {
    if (!expanded.has(seg.file)) {
      out.push(seg);
      continue;
    }
    if (done.has(seg.file)) continue;
    done.add(seg.file);
    const f = files[seg.file];
    const hunks = new Map<number, number[]>();
    f.rows.forEach((_, row) => {
      const h = f.rowHunk[row];
      hunks.set(h, [...(hunks.get(h) ?? []), row]);
    });
    for (const rows of hunks.values()) out.push({ file: seg.file, path: seg.path, rows });
  }
  return out;
}
