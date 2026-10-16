import { describe, expect, it } from "vitest";
import type { FileDto } from "../bindings";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, flatten, hunkKey } from "./model";
import { findLine, lineRef, restoredHunks, savedHunks } from "./savedView";

/** `path` with one hunk starting at each new-side line in `starts`. */
function hunksAt(path: string, starts: number[]): FileDto {
  const f = file(
    path,
    starts.map((n) => rows([" x", "+y"], n, n)),
  );
  f.hunks.forEach((h, i) => {
    h.newStart = starts[i];
  });
  return f;
}

describe("saved hunks", () => {
  it("follow a collapsed hunk to its new index and drop ones that are gone", () => {
    const before = flatten(review([hunksAt("a.rs", [10, 40]), hunksAt("b.rs", [5])]));
    const saved = savedHunks(before, new Set([hunkKey("a.rs", 1), hunkKey("b.rs", 0)]));
    // A hunk added above a.rs's, and b.rs's hunk moved.
    const after = flatten(review([hunksAt("a.rs", [2, 10, 40]), hunksAt("b.rs", [7])]));
    expect(restoredHunks(after, saved)).toEqual(new Set([hunkKey("a.rs", 2)]));
  });
});

describe("findLine", () => {
  it("finds a row by its line number after lines shift above it", () => {
    const before = flatten(review([hunksAt("a.rs", [10])]));
    const beforeLines = buildLines(before, layout());
    const ref = lineRef(before, beforeLines.at(-1));
    const after = flatten(review([hunksAt("new.rs", [1]), hunksAt("a.rs", [3, 10])]));
    const afterLines = buildLines(after, layout());
    const at = findLine(after, afterLines, ref);
    expect(at).toBe(afterLines.length - 1);
    expect(lineRef(after, afterLines[at])).toBe("+11:a.rs");
  });

  it("falls back to the file's header when the row is gone, and to -1 when the file is", () => {
    const before = flatten(review([hunksAt("a.rs", [10])]));
    const ref = lineRef(before, buildLines(before, layout()).at(-1));
    const moved = flatten(review([hunksAt("b.rs", [1]), hunksAt("a.rs", [50])]));
    const movedLines = buildLines(moved, layout());
    expect(movedLines[findLine(moved, movedLines, ref)]).toEqual({ kind: "file", file: 1 });
    const gone = flatten(review([hunksAt("b.rs", [1])]));
    expect(findLine(gone, buildLines(gone, layout()), ref)).toBe(-1);
  });
});
