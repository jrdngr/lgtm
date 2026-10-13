import type { CheckState, MarkDto, NoteDto, StateDto } from "../bindings";
import { commentedRows } from "./comments";
import { type FlatFile, markKey, needsReview } from "./model";
import { hotRows } from "./notes";

export type RowState = "reviewed" | "skipped" | "flagged" | "unreviewed";
export type Filter = "all" | RowState;

export const plural = (n: number) => (n === 1 ? "row" : "rows");

export const FILTERS: Filter[] = ["all", "unreviewed", "reviewed", "skipped", "flagged"];

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

/** A checked file counts as reviewed whatever its line marks say. Unmarked rows in a file
 * matching a `skip` glob count as skipped. Amber rows are unreviewed. */
export function rowStates(files: FlatFile[], index: MarkIndex): RowStateFn {
  return (file, row) => {
    if (index.checks[file] === "checked") return "reviewed";
    const mark = index.marks.get(markKey(file, row));
    if (mark) return mark.state;
    return files[file]?.dto.skipped ? "skipped" : "unreviewed";
  };
}

export interface Progress {
  reviewed: number;
  skipped: number;
  flagged: number;
  unreviewed: number;
  changed: number;
}

export const emptyProgress = (): Progress => ({
  reviewed: 0,
  skipped: 0,
  flagged: 0,
  unreviewed: 0,
  changed: 0,
});

/** Counts by state of the rows that need review, per file. */
export function fileProgress(files: FlatFile[], state: RowStateFn): Progress[] {
  return files.map((f, file) => {
    const p = emptyProgress();
    f.rows.forEach((r, row) => {
      if (!needsReview(r)) return;
      p[state(file, row)]++;
      p.changed++;
    });
    return p;
  });
}

export function sumProgress(all: Progress[]): Progress {
  const out = emptyProgress();
  for (const p of all) {
    for (const k of Object.keys(out) as (keyof Progress)[]) out[k] += p[k];
  }
  return out;
}

/** True when the file has rows the filter shows. */
export function matchesFilter(p: Progress, filter: Filter) {
  return filter === "all" || p[filter] > 0;
}
