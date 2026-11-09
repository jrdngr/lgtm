import { describe, expect, it } from "vitest";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, type DisplayLine, flatten } from "./model";
import { commentAt, commentStep, itemTarget } from "./reviewSteps";

describe("itemTarget", () => {
  const dto = file("a.rs", [rows([" fn a() {}", "+fn b() {}", " fn c() {}", " fn d() {}"])]);
  dto.newSteps = [
    { line: 1, next: 2, prev: null },
    { line: 2, next: 3, prev: 1 },
    { line: 3, next: 4, prev: 2 },
    { line: 4, next: null, prev: 3 },
  ];
  const files = flatten(review([dto]));
  const lines = buildLines(files, layout());
  const at = (n: number) => lines.findIndex((l) => l.kind === "rows" && l.rows[0] === n - 1);

  it("steps to the next and previous item's line", () => {
    expect(itemTarget(files, lines, at(1), 1)).toBe(at(2));
    expect(itemTarget(files, lines, at(3), -1)).toBe(at(2));
    expect(itemTarget(files, lines, at(4), 1)).toBeUndefined();
  });

  it("passes over items the lines don't show", () => {
    const shown = lines.filter((l) => !(l.kind === "rows" && l.rows[0] === 1));
    const first = shown.findIndex((l) => l.kind === "rows" && l.rows[0] === 0);
    const third = shown.findIndex((l) => l.kind === "rows" && l.rows[0] === 2);
    expect(itemTarget(files, shown, first, 1)).toBe(third);
  });
});

describe("comment steps", () => {
  const lines: DisplayLine[] = [
    { kind: "file", file: 0 },
    { kind: "rows", file: 0, hunk: 0, rows: [0] },
    { kind: "comment", file: 0, hunk: 0, comment: 7 },
    { kind: "rows", file: 0, hunk: 0, rows: [1] },
    { kind: "rows", file: 0, hunk: 0, rows: [2] },
    { kind: "comment", file: 0, hunk: 0, comment: 9 },
  ];

  it("finds the comment under the cursor's line", () => {
    expect(commentAt(lines, 1)).toBe(7);
    expect(commentAt(lines, 3)).toBeUndefined();
  });

  it("steps past the cursor's own comment", () => {
    expect(commentStep(lines, 1, 1)).toBe(9);
    expect(commentStep(lines, 3, 1)).toBe(9);
    expect(commentStep(lines, 4, -1)).toBe(7);
    expect(commentStep(lines, 1, -1)).toBeUndefined();
  });
});
