import type { CheckState, MarkColor, MarkDto, NoteDto, StateDto } from "../bindings";
import { commentedRows } from "./comments";
import type { DoneFn, MarkSetup } from "./marks";
import { type FlatFile, markKey, needsReview } from "./model";
import { hotRows } from "./notes";

/** A changed row's mark, or with none: in a checked file, in a skipped file, or unmarked. */
export type RowState = MarkColor | "checked" | "skipped" | "unmarked";
/** Every row, rows that aren't done, or rows with one mark. */
export type Filter = "all" | "left" | MarkColor;

export const plural = (n: number) => (n === 1 ? "row" : "rows");

export function stateDone(s: RowState, done: DoneFn) {
  if (s === "unmarked") return false;
  return s === "checked" || s === "skipped" || done(s);
}

/** Whether a changed row in state `s` shows under `filter`. */
export function rowMatches(s: RowState, filter: Filter, done: DoneFn) {
  if (filter === "all") return true;
  return filter === "left" ? !stateDone(s, done) : s === filter;
}

/** The review's marks, checks, and amber rows, indexed by `markKey`. */
export interface MarkIndex {
  marks: Map<string, MarkDto>;
  checks: CheckState[];
  amber: Set<string>;
  /** Rows inside a comment. */
  commented: Set<string>;
  /** The worst important note on each row. */
  hot: Map<string, NoteDto>;
}

export function indexState(state: StateDto): MarkIndex {
  return {
    marks: new Map(state.marks.map((m) => [markKey(m.file, m.row), m])),
    checks: state.checks,
    amber: new Set(state.amber.map((r) => markKey(r.file, r.row))),
    commented: commentedRows(state.comments),
    hot: hotRows(state.notes),
  };
}

export type RowStateFn = (file: number, row: number) => RowState;

/** A checked file's rows count as checked whatever their line marks say. Unmarked rows in a
 * file matching a `skip` glob count as skipped. Amber rows are unmarked. */
export function rowStates(files: FlatFile[], index: MarkIndex): RowStateFn {
  return (file, row) => {
    if (index.checks[file] === "checked") return "checked";
    const mark = index.marks.get(markKey(file, row));
    if (mark) return mark.color;
    return files[file]?.dto.skipped ? "skipped" : "unmarked";
  };
}

export interface Progress {
  /** Rows by mark. */
  marks: Partial<Record<MarkColor, number>>;
  /** Unmarked rows done by their file: checked, or matching a `skip` glob. */
  byFile: number;
  done: number;
  changed: number;
}

export const emptyProgress = (): Progress => ({ marks: {}, byFile: 0, done: 0, changed: 0 });

/** Counts of the rows that need review, per file. */
export function fileProgress(files: FlatFile[], state: RowStateFn, done: DoneFn): Progress[] {
  return files.map((f, file) => {
    const p = emptyProgress();
    f.rows.forEach((r, row) => {
      if (!needsReview(r)) return;
      const s = state(file, row);
      if (s === "checked" || s === "skipped") p.byFile++;
      else if (s !== "unmarked") p.marks[s] = (p.marks[s] ?? 0) + 1;
      if (stateDone(s, done)) p.done++;
      p.changed++;
    });
    return p;
  });
}

export function sumProgress(all: Progress[]): Progress {
  const out = emptyProgress();
  for (const p of all) {
    for (const [c, n] of Object.entries(p.marks) as [MarkColor, number][])
      out.marks[c] = (out.marks[c] ?? 0) + n;
    out.byFile += p.byFile;
    out.done += p.done;
    out.changed += p.changed;
  }
  return out;
}

/** True when the file has rows the filter shows. */
export function matchesFilter(p: Progress, filter: Filter) {
  if (filter === "all") return true;
  return filter === "left" ? p.done < p.changed : (p.marks[filter] ?? 0) > 0;
}

/** A filter as words, e.g. "not done" or a mark's label. */
export function filterLabel(filter: Filter, setup: MarkSetup) {
  if (filter === "all") return "all lines";
  return filter === "left" ? "not done" : setup.label(filter);
}

/** The counts behind a progress figure, for a tooltip. */
export function progressTitle(p: Progress, setup: MarkSetup) {
  const parts = setup.slots
    .filter((s) => (p.marks[s.color] ?? 0) > 0)
    .map((s) => `${p.marks[s.color]} ${setup.label(s.color)}`);
  if (p.byFile > 0) parts.push(`${p.byFile} in checked or skipped files`);
  parts.push(`${p.changed - p.done} left`);
  return parts.join(", ");
}
