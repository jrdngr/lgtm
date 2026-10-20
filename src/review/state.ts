import type { CheckState, MarkColor, MarkDto, StateDto } from "../bindings";
import { commentedRows } from "./comments";
import type { DoneFn, MarkSetup } from "./marks";
import { type FlatFile, markKey, needsReview } from "./model";
import { type Filter, type RowStateFn, stateDone } from "./rowState";

export const plural = (n: number) => (n === 1 ? "row" : "rows");

/** The review's marks, checks, and amber rows, indexed by `markKey`. */
export interface MarkIndex {
  marks: Map<string, MarkDto>;
  checks: CheckState[];
  amber: Set<string>;
  /** Rows inside a comment. */
  commented: Set<string>;
}

export function indexState(state: StateDto): MarkIndex {
  return {
    marks: new Map(state.marks.map((m) => [markKey(m.file, m.row), m])),
    checks: state.checks,
    amber: new Set(state.amber.map((r) => markKey(r.file, r.row))),
    commented: commentedRows(state.comments),
  };
}

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

/** One part of the progress bar: rows marked one color, rows done by their file, or
 * unmarked rows. */
export interface ProgressPart {
  key: string;
  label: string;
  color?: MarkColor;
  rows: number;
  done: boolean;
}

/** The parts with rows, marks in setup order, then rows done by their file, then unmarked. */
export function progressParts(p: Progress, setup: MarkSetup): ProgressPart[] {
  const marked = setup.slots
    .filter((s) => (p.marks[s.color] ?? 0) > 0)
    .map((s) => ({
      key: s.color,
      label: setup.label(s.color),
      color: s.color,
      rows: p.marks[s.color] ?? 0,
      done: setup.done(s.color),
    }));
  const unmarked = p.changed - p.byFile - marked.reduce((n, m) => n + m.rows, 0);
  return [
    ...marked,
    { key: "byFile", label: "checked files", rows: p.byFile, done: true },
    { key: "unmarked", label: "unmarked", rows: unmarked, done: false },
  ].filter((x) => x.rows > 0);
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
