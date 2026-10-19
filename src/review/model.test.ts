import { describe, expect, it } from "vitest";
import type { CheckState, MarkColor } from "../bindings";
import { file, layout, review, rows } from "./fixture.test-utils";
import {
  buildLines,
  changedRows,
  changeTargets,
  commentJumpTarget,
  type DisplayLine,
  fileJumpTarget,
  fileTargets,
  flatten,
  hunkKey,
  regionKey,
  regions,
  stepTarget,
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

  it("shows only rows with the filter's mark, with gap bars for the rest", () => {
    const flagged = (_: number, row: number) => (row === 5 ? "red" : "unmarked");
    const lines = buildLines(files, layout({ filter: "red", rowState: flagged }));
    expect(
      lines.map((l) => (l.kind === "rows" || l.kind === "gap" ? [l.kind, l.rows] : l.kind)),
    ).toEqual(["file", "hunk", ["gap", [0, 1, 2, 3, 4]], ["rows", [5]]]);
  });

  it("shows a change's removed rows with its added rows, which alone take the state", () => {
    const marked = (_: number, row: number) => (row === 3 ? "unmarked" : "cyan");
    const lines = buildLines(files, layout({ filter: "left", rowState: marked }));
    expect(
      lines.map((l) => (l.kind === "rows" || l.kind === "gap" ? [l.kind, l.rows] : l.kind)),
    ).toEqual([
      "file",
      "hunk",
      ["gap", [0]],
      ["rows", [1]],
      ["rows", [2]],
      ["rows", [3]],
      ["gap", [4, 5]],
    ]);
  });

  it("shows a revealed gap's rows and hides files with nothing to show", () => {
    const flagged = (_: number, row: number) => (row === 3 ? "red" : "unmarked");
    const lines = buildLines(
      files,
      layout({ filter: "red", rowState: flagged, revealed: new Set(["0:0:0"]) }),
    );
    expect(lines.filter((l) => l.kind === "rows").map((l) => l.rows)).toEqual([[0], [1], [2], [3]]);
    expect(buildLines(files, layout({ filter: "gray" }))).toEqual([]);
  });

  it("puts comment blocks after their row, and gone ones under the file header", () => {
    const lines = buildLines(
      files,
      layout({
        comments: new Map([
          ["0:1", [5, 8]],
          ["0:file", [9]],
        ]),
      }),
    );
    const blocks = lines.flatMap((l, i) => (l.kind === "comment" ? [[i, l.comment]] : []));
    expect(blocks).toEqual([
      [1, 9],
      [5, 5],
      [6, 8],
    ]);
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

  it("takes collapsed rows only when asked, leaving out rows the filter hides", () => {
    const flagged = (_: number, row: number) => (row === 5 ? "red" : "unmarked");
    const rowsOf = (lines: DisplayLine[], folded: boolean) =>
      changedRows(files, lines, 0, lines.length - 1, folded).map((a) => a.row);
    const file = buildLines(files, layout({ collapsedFiles: new Set(["a.rs"]) }));
    expect(rowsOf(file, false)).toEqual([]);
    expect(rowsOf(file, true)).toEqual([1, 2, 3, 5]);
    const hunk = buildLines(
      files,
      layout({ collapsedHunks: new Set([hunkKey("a.rs", 0)]), filter: "red", rowState: flagged }),
    );
    expect(rowsOf(hunk, true)).toEqual([5]);
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

describe("change stepping", () => {
  // One long hunk with two edits, as prose often diffs, then a collapsed hunk, then a file.
  const stepFiles = flatten(
    review([
      file("doc.md", [rows([" a", "-b", "+B", " c", " d", "+e", " f"]), rows(["+g"])]),
      file("b.rs", [rows([" x", "-y"])]),
    ]),
  );
  const lines = buildLines(stepFiles, layout({ collapsedHunks: new Set([hunkKey("doc.md", 1)]) }));
  const targets = changeTargets(stepFiles, lines);
  const step = (cursor: number, dir: 1 | -1) => stepTarget(targets, cursor, dir)?.cursor;

  it("stops at each run of changed rows, or a collapsed hunk's header", () => {
    // 0 file, 1 hunk, 2-8 rows, 9 hunk (collapsed), 10 file, 11 hunk, 12-13 rows
    expect(targets).toEqual([
      { cursor: 3, top: 0 },
      { cursor: 7, top: 4 },
      { cursor: 9, top: 7 },
      { cursor: 13, top: 9 },
    ]);
  });

  it("goes back from inside a change and stops at the ends", () => {
    expect(step(5, 1)).toBe(7);
    expect(step(5, -1)).toBe(3);
    expect(step(13, 1)).toBeUndefined();
    expect(step(3, -1)).toBeUndefined();
  });

  it("steps files the same way, going back to the current file's start first", () => {
    const fileStops = fileTargets(lines);
    expect(fileStops.map((t) => t.cursor)).toEqual([2, 12]);
    expect(stepTarget(fileStops, 5, 1)?.cursor).toBe(12);
    expect(stepTarget(fileStops, 5, -1)?.cursor).toBe(2);
  });
});

describe("fileProgress", () => {
  const mark = (row: number, color: MarkColor) => ({
    file: 0,
    row,
    color,
    review: "r",
    createdAt: 0,
  });
  const indexed = (marks: ReturnType<typeof mark>[], check: CheckState = "unchecked") =>
    indexState({ marks, checks: [check], amber: [], comments: [], threads: [] });
  const done = (c: MarkColor) => c !== "red";

  it("counts rows that need review by mark, leaving out a change's removed rows", () => {
    const index = indexed([mark(1, "cyan"), mark(3, "red")]);
    expect(fileProgress(files, rowStates(files, index), done)).toEqual([
      { marks: { red: 1 }, byFile: 0, done: 0, changed: 2 },
    ]);
    const deletion = flatten(review([file("b.rs", [rows([" a", "-b", " c"])])]));
    expect(fileProgress(deletion, rowStates(deletion, indexed([])), done)[0].done).toBe(0);
  });

  it("counts a checked file as done and a skip-glob file's unmarked rows as done", () => {
    const checked = indexed([mark(3, "red")], "checked");
    expect(fileProgress(files, rowStates(files, checked), done)[0]).toMatchObject({
      byFile: 2,
      done: 2,
    });
    const skipFiles = flatten(review([{ ...files[0].dto, skipped: true }]));
    const none = indexed([mark(3, "red")]);
    expect(fileProgress(skipFiles, rowStates(skipFiles, none), done)[0]).toMatchObject({
      marks: { red: 1 },
      done: 1,
      changed: 2,
    });
  });
});

describe("hidden unchanged code", () => {
  // Hunks at new lines 5-7 and 20-21 of a 30-line file, the second shifted by one removed line.
  const dto = {
    ...file("b.rs", []),
    lineCount: 30,
    hunks: [
      {
        oldStart: 5,
        oldLines: 2,
        newStart: 5,
        newLines: 3,
        section: "",
        rows: rows([" a", "+b", " c"], 5, 5),
      },
      {
        oldStart: 19,
        oldLines: 2,
        newStart: 20,
        newLines: 2,
        section: "",
        rows: rows([" d", " e"], 19, 20),
      },
    ],
  };
  const b = flatten(review([dto]));

  it("sits before each hunk and after the last, with the shift to old line numbers", () => {
    expect([...regions(dto)]).toEqual([
      [0, { from: 1, to: 4, shift: 0 }],
      [1, { from: 8, to: 19, shift: -1 }],
      [2, { from: 22, to: 30, shift: -1 }],
    ]);
  });

  it("shows as a bar until opened, then as its lines, and not under a filter", () => {
    const kinds = (l: ReturnType<typeof layout>) => buildLines(b, l).map((x) => x.kind);
    expect(kinds(layout())).toEqual([
      "file",
      "more",
      "hunk",
      "rows",
      "rows",
      "rows",
      "more",
      "hunk",
      "rows",
      "rows",
      "more",
    ]);
    const line = { line: 8, text: "x", hl: null };
    const opened = buildLines(
      b,
      layout({ context: new Map([[regionKey("b.rs", { from: 8, to: 19, shift: -1 }), [line]]]) }),
    );
    expect(opened[6]).toEqual({ kind: "context", file: 0, hunk: 1, line, old: 7 });
    expect(kinds(layout({ filter: "red" }))).not.toContain("more");
  });
});
