import { describe, expect, it } from "vitest";
import { file, layout, review, rows } from "./fixture.test-utils";
import { firstLine, freshRows } from "./followChanges";
import { buildLines, flatten } from "./model";

describe("freshRows", () => {
  it("finds only the rows a reload added, counting repeated lines", () => {
    const before = flatten(review([file("a.rs", [rows(["+}", " x", "-old"])])]));
    const after = flatten(
      review([file("a.rs", [rows(["+}", " x", "-old", "+}"])]), file("b.rs", [rows(["+}"])])]),
    );
    expect([...freshRows(before, after)].sort()).toEqual(["0:3", "1:0"]);
  });

  it("scrolls to the first line showing a fresh row", () => {
    const files = flatten(review([file("a.rs", [rows([" a", "+b", "+c"])])]));
    const lines = buildLines(files, layout());
    const line = firstLine(lines, new Set(["0:2"])) ?? -1;
    expect(lines[line]).toMatchObject({ kind: "rows", rows: [2] });
  });
});
