import { expect, test } from "vitest";
import { toggledMark } from "./model";
import type { RowState } from "./rowState";

const marks: Record<string, RowState> = { "0:1": "cyan", "0:2": "cyan", "0:3": "red" };
const state = (file: number, row: number) => marks[`${file}:${row}`] ?? "unmarked";
const rows = (...r: number[]) => r.map((row) => ({ file: 0, row }));

test("a mark every row already has clears instead", () => {
  expect(toggledMark(rows(1, 2), "cyan", state)).toBeNull();
});

test("a mark some rows lack is set on all of them", () => {
  expect(toggledMark(rows(1, 2, 3), "cyan", state)).toBe("cyan");
  expect(toggledMark(rows(1, 4), "cyan", state)).toBe("cyan");
});
