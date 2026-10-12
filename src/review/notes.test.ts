import { expect, test } from "vitest";
import type { NoteDto } from "../bindings";
import { hotRows, nextStep, noteSlots, tours } from "./notes";

const note = (
  id: number,
  rows: [number, number] | null,
  extra: Partial<NoteDto> = {},
): NoteDto => ({
  id,
  path: "a.rs",
  file: 0,
  rows: rows && { start: rows[0], end: rows[1] },
  category: "context",
  level: null,
  title: `note ${id}`,
  body: "",
  flow: null,
  position: id,
  source: "headless",
  ...extra,
});

test("the gutter shows the worst important note on each row and ignores the rest", () => {
  const rows = hotRows([
    note(1, [1, 3], { category: "important", level: "low" }),
    note(2, [3, 4], { category: "important", level: "high" }),
    note(3, [0, 5], { category: "context" }),
  ]);
  expect(rows.get("0:1")?.id).toBe(1);
  expect(rows.get("0:3")?.id).toBe(2);
  expect(rows.has("0:0")).toBe(false);
});

test("a note's block follows its last row, or its file header when its code is gone", () => {
  const slots = noteSlots([note(1, [1, 3]), note(2, [2, 3]), note(3, null)]);
  expect(slots.get("0:3")).toEqual([1, 2]);
  expect(slots.get("0:file")).toEqual([3]);
});

test("each flow is a tour in step order, after the reading order", () => {
  const step = (flow: string, n: number) => ({
    flow: { flow, step: n },
    category: "flow" as const,
  });
  const notes = [
    note(0, [0, 0]),
    note(1, [1, 1], step("Login", 2)),
    note(2, [2, 2], step("Save", 1)),
    note(3, [3, 3], step("Login", 1)),
  ];
  expect(tours(notes).map((t) => [t.name, t.notes.map((n) => n.id)])).toEqual([
    ["Reading order", [0, 1, 2, 3]],
    ["Login", [3, 1]],
    ["Save", [2]],
  ]);
});

test("stepping starts at either end and wraps around", () => {
  expect(nextStep(3, undefined, 1)).toBe(0);
  expect(nextStep(3, undefined, -1)).toBe(2);
  expect(nextStep(3, 2, 1)).toBe(0);
  expect(nextStep(3, 0, -1)).toBe(2);
  expect(nextStep(0, undefined, 1)).toBeUndefined();
});
