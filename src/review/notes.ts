import type { Category, NoteDto } from "../bindings";
import { fileSlot, markKey } from "./model";

export const CATEGORIES: Record<Category, { emoji: string; label: string }> = {
  important: { emoji: "⚠️", label: "Important" },
  flow: { emoji: "🧭", label: "Flow step" },
  context: { emoji: "💡", label: "Context" },
  question: { emoji: "❓", label: "Question" },
};

export const CATEGORY_ORDER: Category[] = ["important", "flow", "context", "question"];

const LEVELS = ["high", "medium", "low"];

/** The worst important note on each row, by `markKey`. */
export function hotRows(notes: NoteDto[]): Map<string, NoteDto> {
  const out = new Map<string, NoteDto>();
  const rank = (n: NoteDto) => LEVELS.indexOf(n.level ?? "low");
  for (const n of notes) {
    if (n.category !== "important" || !n.rows) continue;
    for (let r = n.rows.start; r <= n.rows.end; r++) {
      const key = markKey(n.file, r);
      const had = out.get(key);
      if (!had || rank(n) < rank(had)) out.set(key, n);
    }
  }
  return out;
}

/** Note ids by the `markKey` of the row their block follows, or by `fileSlot` for ones whose
 * code is gone. */
export function noteSlots(notes: NoteDto[]): Map<string, number[]> {
  const out = new Map<string, number[]>();
  for (const n of notes) {
    const key = n.rows ? markKey(n.file, n.rows.end) : fileSlot(n.file);
    out.set(key, [...(out.get(key) ?? []), n.id]);
  }
  return out;
}

/** A path through some of the notes: every note in reading order, or one flow's steps. */
export interface Tour {
  id: string;
  name: string;
  notes: NoteDto[];
}

export const READING_ORDER = "reading";

/** The reading order, then each flow in the order it starts. */
export function tours(notes: NoteDto[]): Tour[] {
  const flows = new Map<string, NoteDto[]>();
  for (const n of notes) {
    if (n.flow) flows.set(n.flow.flow, [...(flows.get(n.flow.flow) ?? []), n]);
  }
  return [
    { id: READING_ORDER, name: "Reading order", notes },
    ...[...flows].map(([name, steps]) => ({
      id: `flow:${name}`,
      name,
      notes: [...steps].sort((a, b) => (a.flow?.step ?? 0) - (b.flow?.step ?? 0)),
    })),
  ];
}

/** The step `dir` away from `at` in a tour of `length` notes, wrapping around. Starts at the
 * first or last note when there's no step yet. */
export function nextStep(length: number, at: number | undefined, dir: 1 | -1): number | undefined {
  if (length === 0) return undefined;
  if (at === undefined) return dir === 1 ? 0 : length - 1;
  return (at + dir + length) % length;
}
