import { describe, expect, it } from "vitest";
import { file, layout, review, rows } from "./fixture.test-utils";
import { follow, movesFrom } from "./follow";
import { buildLines, flatten } from "./model";
import { lineSelection, rangeSelection } from "./selection";

const files = flatten(review([file("a.rs", [rows(["+a", "+b", "+c", "+d"])])]));
const all = buildLines(files, layout());
const lineOf = (lines: typeof all, row: number) =>
  lines.findIndex((l) => l.kind === "rows" && l.rows.includes(row));

describe("follow", () => {
  it("moves the cursor to the next shown row when its row leaves the filter", () => {
    const reviewed = new Set([1]);
    const unreviewed = buildLines(
      files,
      layout({
        filter: "unreviewed",
        rowState: (_, r) => (reviewed.has(r) ? "reviewed" : "unreviewed"),
      }),
    );
    const before = buildLines(files, layout({ filter: "unreviewed" }));
    const at = lineOf(before, 1);
    const next = follow(before, unreviewed, { sel: lineSelection(at), cursor: at, anchor: at });
    expect(unreviewed[next.cursor]).toMatchObject({ kind: "rows", rows: [2] });
    expect(next.sel.level).toBe("line");
  });

  it("keeps a selection on its rows after a refresh and shrinks it to what's left", () => {
    // Row 0 was removed from the diff; rows 1..3 moved up by one.
    const after = flatten(review([file("a.rs", [rows(["+b", "+c", "+d"])])]));
    const afterLines = buildLines(after, layout());
    const moves = movesFrom(
      [1, 2, 3].map((r) => ({ from: { file: 0, row: r }, to: { file: 0, row: r - 1 } })),
      [[0, 0]],
    );
    const sel = rangeSelection(lineOf(all, 0), lineOf(all, 2));
    const next = follow(all, afterLines, { sel, cursor: sel.end, anchor: sel.start }, moves);
    expect([next.sel.start, next.sel.end]).toEqual([lineOf(afterLines, 0), lineOf(afterLines, 1)]);
    expect(next.cursor).toBe(lineOf(afterLines, 1));
  });

  it("clears the selection to the cursor when all of its rows are gone", () => {
    const after = flatten(review([file("a.rs", [rows(["+x"])])]));
    const afterLines = buildLines(after, layout());
    const sel = rangeSelection(lineOf(all, 1), lineOf(all, 2));
    const next = follow(
      all,
      afterLines,
      { sel, cursor: sel.end, anchor: sel.start },
      movesFrom([], []),
    );
    expect(next.sel.level).toBe("line");
    expect(next.sel.start).toBe(next.cursor);
  });
});
