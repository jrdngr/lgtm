import { describe, expect, it } from "vitest";
import { file, layout, review, rows } from "../review/fixture.test-utils";
import { buildLines, flatten } from "../review/model";
import { findInLines } from "./reviewFind";

const files = flatten(review([file("a.rs", [rows([" let a", "-let b", "+let B", " a + B"])])]));

describe("findInLines", () => {
  it("finds every match in shown code, numbering each line's matches", () => {
    const lines = buildLines(files, layout());
    const found = findInLines(files, lines, "unified", "b", false);
    const texts = found.map((m) => lines[m.line]);
    expect(found).toHaveLength(3);
    expect(texts.every((l) => l.kind === "rows")).toBe(true);
    expect(found.map((m) => m.nth)).toEqual([0, 0, 0]);
    expect(findInLines(files, lines, "unified", "B", true)).toHaveLength(2);
  });

  it("counts a context row on both sides in split mode, as it shows", () => {
    const lines = buildLines(files, layout({ mode: "split" }));
    const found = findInLines(files, lines, "split", "let a", false);
    expect(found).toEqual([
      { line: found[0].line, nth: 0 },
      { line: found[0].line, nth: 1 },
    ]);
  });
});
