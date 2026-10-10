import { describe, expect, it } from "vitest";
import { file, review, rows, sym } from "./fixture.test-utils";
import { buildLines, flatten } from "./model";
import { expand, lineSelection, units } from "./selection";

// Line 1 `fn outer`, line 2 `fn inner` (one line), lines 3-4 body, line 5 broken symbol.
const files = flatten(
  review([
    file(
      "a.rs",
      [rows([" fn outer() {", "+  fn inner() {}", "+  x();", " }", "+fn broken( {"])],
      [sym("outer", 1, 4), sym("inner", 2, 2), sym("broken", 5, 5, true)],
    ),
    file("b.rs", [rows(["+only"])]),
  ]),
);
const lines = buildLines(files, "unified", new Set());
const at = (text: string) =>
  lines.findIndex((l) => l.kind === "rows" && files[l.file].rows[l.rows[0]].text === text);

describe("units", () => {
  it("orders innermost function, outer function, hunk, file, review", () => {
    expect(units(files, lines, at("  fn inner() {}")).map((u) => u.label)).toEqual([
      "function: inner",
      "function: outer",
      "hunk",
      "file: a.rs",
      "review",
    ]);
  });

  it("skips functions whose parse has errors", () => {
    expect(units(files, lines, at("fn broken( {")).map((u) => u.level)).toEqual([
      "hunk",
      "file",
      "review",
    ]);
  });
});

describe("expand", () => {
  it("steps to the next strictly larger unit and skips units equal to the selection", () => {
    const cursor = at("  x();");
    const steps: string[] = [];
    for (
      let sel = expand(files, lines, cursor, lineSelection(cursor));
      sel;
      sel = expand(files, lines, cursor, sel)
    ) {
      steps.push(sel.label);
    }
    expect(steps).toEqual(["function: outer", "hunk", "file: a.rs", "review"]);
  });

  it("expands a line outside any symbol straight to its hunk", () => {
    const cursor = at("only");
    expect(expand(files, lines, cursor, lineSelection(cursor))?.level).toBe("hunk");
  });
});
