import type { MarkColor } from "../bindings";
import type { DoneFn } from "./marks";

/** A changed row's mark, or with none: in a checked file, in a skipped file, or unmarked. */
export type RowState = MarkColor | "checked" | "skipped" | "unmarked";
/** Every row, rows that aren't done, or rows with one mark. */
export type Filter = "all" | "left" | MarkColor;

export type RowStateFn = (file: number, row: number) => RowState;

export function stateDone(s: RowState, done: DoneFn) {
  if (s === "unmarked") return false;
  return s === "checked" || s === "skipped" || done(s);
}

/** Whether a changed row in state `s` shows under `filter`. */
export function rowMatches(s: RowState, filter: Filter, done: DoneFn) {
  if (filter === "all") return true;
  return filter === "left" ? !stateDone(s, done) : s === filter;
}
