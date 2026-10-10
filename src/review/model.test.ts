import { describe, expect, it } from "vitest";
import { file, layout, review, rows, sym } from "./fixture.test-utils";
import { buildLines, changedRows, flatten, hunkKey, symbolMarkers } from "./model";
import { fileProgress, indexState, rowStates } from "./state";

const r = review([file("a.rs", [rows([" a", "-b", "-c", "+B", " d", "+e"])])]);
const files = flatten(r);

describe("buildLines", () => {
  it("pairs removed runs with following added runs in split view", () => {
    const lines = buildLines(files, layout({ mode: "split" })).filter((l) => l.kind === "rows");
    expect(lines.map((l) => [l.left, l.right])).toEqual([
      [0, 0],
      [1, 3],
      [2, undefined],
      [4, 4],
      [undefined, 5],
    ]);
  });

  it("drops a collapsed hunk's rows but keeps its header", () => {
    const lines = buildLines(files, layout({ collapsedHunks: new Set([hunkKey("a.rs", 0)]) }));
    expect(lines.map((l) => l.kind)).toEqual(["file", "hunk"]);
  });

  it("shows only rows in the filter's state, with gap bars for the rest", () => {
    const flagged = (_: number, row: number) => (row === 2 ? "flagged" : "unreviewed");
    const lines = buildLines(files, layout({ filter: "flagged", rowState: flagged }));
    expect(
      lines.map((l) => (l.kind === "rows" || l.kind === "gap" ? [l.kind, l.rows] : l.kind)),
    ).toEqual(["file", "hunk", ["gap", [0, 1]], ["rows", [2]], ["gap", [3, 4, 5]]]);
  });

  it("shows a revealed gap's rows and hides files with nothing to show", () => {
    const flagged = (_: number, row: number) => (row === 2 ? "flagged" : "unreviewed");
    const lines = buildLines(
      files,
      layout({ filter: "flagged", rowState: flagged, revealed: new Set(["0:0:0"]) }),
    );
    expect(lines.filter((l) => l.kind === "rows").map((l) => l.rows)).toEqual([[0], [1], [2]]);
    expect(buildLines(files, layout({ filter: "skipped" }))).toEqual([]);
  });

  it("shows only the header of a collapsed file", () => {
    const lines = buildLines(files, layout({ collapsedFiles: new Set(["a.rs"]) }));
    expect(lines.map((l) => l.kind)).toEqual(["file"]);
  });
});

describe("changedRows", () => {
  it("skips context rows and covers both sides of split lines", () => {
    const lines = buildLines(files, layout({ mode: "split" }));
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
  const mark = (row: number, state: "reviewed" | "flagged") => ({
    file: 0,
    row,
    state,
    review: "r",
    createdAt: 0,
  });

  it("counts changed rows by mark state", () => {
    const index = indexState({
      marks: [mark(1, "reviewed"), mark(2, "flagged")],
      checks: ["unchecked"],
      amber: [],
    });
    expect(fileProgress(files, rowStates(files, index))).toEqual([
      { reviewed: 1, skipped: 0, flagged: 1, unreviewed: 2, changed: 4 },
    ]);
  });

  it("counts a checked file as fully reviewed and a skip-glob file as skipped", () => {
    const checked = indexState({ marks: [mark(1, "flagged")], checks: ["checked"], amber: [] });
    expect(fileProgress(files, rowStates(files, checked))[0].reviewed).toBe(4);
    const skipFiles = flatten(review([{ ...files[0].dto, skipped: true }]));
    const none = indexState({ marks: [mark(1, "flagged")], checks: ["unchecked"], amber: [] });
    expect(fileProgress(skipFiles, rowStates(skipFiles, none))[0]).toMatchObject({
      skipped: 3,
      flagged: 1,
    });
  });
});
