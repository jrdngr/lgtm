import { expect, test } from "vitest";
import { toggledMark } from "./model";
import type { RowState } from "./rowState";

const marks: Record<string, RowState> = { "0:1": "green", "0:2": "green", "0:3": "red" };
const state = (file: number, row: number) => marks[`${file}:${row}`] ?? "unmarked";
const rows = (...r: number[]) => r.map((row) => ({ file: 0, row }));

test("a mark every row already has clears instead", () => {
  expect(toggledMark(rows(1, 2), "green", state)).toBeNull();
});

test("a mark some rows lack is set on all of them", () => {
  expect(toggledMark(rows(1, 2, 3), "green", state)).toBe("green");
  expect(toggledMark(rows(1, 4), "green", state)).toBe("green");
});
