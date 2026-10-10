import { describe, expect, it } from "vitest";
import { file, review, rows, sym } from "./fixture.test-utils";
import { buildLines, changedRows, fileProgress, flatten, hunkKey, symbolMarkers } from "./model";

const r = review([file("a.rs", [rows([" a", "-b", "-c", "+B", " d", "+e"])])]);
const files = flatten(r);

describe("buildLines", () => {
  it("pairs removed runs with following added runs in split view", () => {
    const lines = buildLines(files, "split", new Set()).filter((l) => l.kind === "rows");
    expect(lines.map((l) => [l.left, l.right])).toEqual([
      [0, 0],
      [1, 3],
      [2, undefined],
      [4, 4],
      [undefined, 5],
    ]);
  });

  it("drops a collapsed hunk's rows but keeps its header", () => {
    const lines = buildLines(files, "unified", new Set([hunkKey(0, 0)]));
    expect(lines.map((l) => l.kind)).toEqual(["file", "hunk"]);
  });
});

describe("changedRows", () => {
  it("skips context rows and covers both sides of split lines", () => {
    const lines = buildLines(files, "split", new Set());
    expect(changedRows(files, lines, 0, lines.length - 1).map((a) => a.row)).toEqual([1, 3, 2, 5]);
  });
});

describe("symbolMarkers", () => {
  it("puts one marker on the first changed row of each changed symbol, innermost first", () => {
    const f = flatten(
      review([
        file(
          "a.rs",
          [rows([" fn outer() {", "+  fn inner() {}", "+  x();", " }"])],
          [sym("outer", 1, 4), sym("inner", 2, 2)],
        ),
      ]),
    )[0];
    const markers = symbolMarkers(f);
    expect([...markers].map(([row, m]) => [row, m.symbol.name])).toEqual([[1, "inner"]]);
  });
});

describe("fileProgress", () => {
  it("counts changed rows and reviewed marks only", () => {
    const mark = (row: number, state: "reviewed" | "flagged") => ({
      file: 0,
      row,
      state,
      review: "r",
      createdAt: null,
    });
    expect(fileProgress(files, [mark(1, "reviewed"), mark(2, "flagged")])).toEqual([
      { reviewed: 1, changed: 4 },
    ]);
  });
});
