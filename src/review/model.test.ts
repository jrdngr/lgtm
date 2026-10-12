import { describe, expect, it } from "vitest";
import type { CheckState } from "../bindings";
import { file, layout, review, rows, sym } from "./fixture.test-utils";
import {
  buildLines,
  changedRows,
  commentJumpTarget,
  type DisplayLine,
  fileJumpTarget,
  flatten,
  hunkKey,
  hunkTargets,
  stepTarget,
  symbolMarkers,
} from "./model";
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

describe("jump targets", () => {
  const lines: DisplayLine[] = [
    { kind: "file", file: 0 },
    { kind: "comment", file: 0, hunk: -1, comment: 7 },
    { kind: "hunk", file: 0, hunk: 0 },
    { kind: "rows", file: 0, hunk: 0, rows: [0] },
    { kind: "comment", file: 0, hunk: 0, comment: 8 },
    { kind: "file", file: 1 },
    { kind: "file", file: 2 },
  ];

  it("puts the cursor on a file's first row, or its header when it's collapsed", () => {
    expect(fileJumpTarget(lines, 0)).toEqual({ cursor: 3, top: 0 });
    expect(fileJumpTarget(lines, 1)).toEqual({ cursor: 5, top: 5 });
    expect(fileJumpTarget(lines, 3)).toBeUndefined();
  });

  it("puts the cursor on the row a comment follows, or the first row under a file block", () => {
    expect(commentJumpTarget(lines, 8)).toEqual({ cursor: 3, top: 1 });
    expect(commentJumpTarget(lines, 7)).toEqual({ cursor: 3, top: 0 });
    expect(commentJumpTarget(lines, 9)).toBeUndefined();
  });
});

describe("hunk stepping", () => {
  const lines: DisplayLine[] = [
    { kind: "file", file: 0 },
    { kind: "hunk", file: 0, hunk: 0 },
    { kind: "rows", file: 0, hunk: 0, rows: [0] },
    { kind: "rows", file: 0, hunk: 0, rows: [1] },
    { kind: "hunk", file: 0, hunk: 1 },
    { kind: "file", file: 1 },
    { kind: "hunk", file: 1, hunk: 0 },
    { kind: "gap", file: 1, hunk: 0, rows: [0] },
    { kind: "rows", file: 1, hunk: 0, rows: [1] },
  ];
  const targets = hunkTargets(lines);
  const step = (cursor: number, dir: 1 | -1) => stepTarget(targets, cursor, dir)?.cursor;

  it("lands on a hunk's first shown row, or its header when it's collapsed", () => {
    expect(targets).toEqual([
      { cursor: 2, top: 1 },
      { cursor: 4, top: 4 },
      { cursor: 8, top: 6 },
    ]);
  });

  it("goes back to the previous hunk from inside one and stops at the ends", () => {
    expect(step(3, 1)).toBe(4);
    expect(step(4, 1)).toBe(8);
    expect(step(8, 1)).toBeUndefined();
    expect(step(3, -1)).toBe(2);
    expect(step(8, -1)).toBe(4);
    expect(step(2, -1)).toBeUndefined();
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
  const indexed = (marks: ReturnType<typeof mark>[], check: CheckState = "unchecked") =>
    indexState({ marks, checks: [check], amber: [], comments: [], hotspots: [] });

  it("counts changed rows by mark state", () => {
    const index = indexed([mark(1, "reviewed"), mark(2, "flagged")]);
    expect(fileProgress(files, rowStates(files, index))).toEqual([
      { reviewed: 1, skipped: 0, flagged: 1, unreviewed: 2, changed: 4 },
    ]);
  });

  it("counts a checked file as fully reviewed and a skip-glob file as skipped", () => {
    const checked = indexed([mark(1, "flagged")], "checked");
    expect(fileProgress(files, rowStates(files, checked))[0].reviewed).toBe(4);
    const skipFiles = flatten(review([{ ...files[0].dto, skipped: true }]));
    const none = indexed([mark(1, "flagged")]);
    expect(fileProgress(skipFiles, rowStates(skipFiles, none))[0]).toMatchObject({
      skipped: 3,
      flagged: 1,
    });
  });
});
