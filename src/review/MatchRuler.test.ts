import { describe, expect, it } from "vitest";
import { file, layout, review, rows } from "./fixture.test-utils";
import { matchedLines } from "./MatchRuler";
import { buildLines, flatten } from "./model";

const files = flatten(
  review([
    file("a.go", [rows([" func a()", "-func b()", "+fn c()", " functional()", "+x := func"])]),
  ]),
);

/** The text of each matched line's rows, in order. */
function matched(mode: "unified" | "split", needle: string): string[][] {
  const lines = buildLines(files, layout({ mode }));
  return matchedLines(files, lines, needle).map((i) => {
    const l = lines[i];
    if (l.kind !== "rows") return [l.kind];
    const shown = mode === "split" ? [l.left, l.right] : l.rows;
    return shown.flatMap((r) => (r === undefined ? [] : [files[0].rows[r].text]));
  });
}

describe("matchedLines", () => {
  it("finds whole-word matches on rows, skipping headers and longer words", () => {
    expect(matched("unified", "func")).toEqual([["func a()"], ["func b()"], ["x := func"]]);
  });

  it("counts a split line when either side matches", () => {
    expect(matched("split", "fn")).toEqual([["func b()", "fn c()"]]);
  });
});
