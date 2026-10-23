import { describe, expect, it } from "vitest";
import type { RowDto } from "../bindings";
import { diffBlocks, lineMap, mapBlocks } from "./diffBlocks";

const row = (kind: RowDto["kind"], text: string, oldLine: number | null, newLine: number | null) =>
  ({ kind, text, oldLine, newLine, replaced: false, hl: null }) as RowDto;

describe("diffBlocks", () => {
  it("tells added, changed and removed runs apart by what they replace", () => {
    const rows = [
      row("context", "a", 1, 1),
      row("removed", "b", 2, null),
      row("added", "B", null, 2),
      row("context", "c", 3, 3),
      row("removed", "d", 4, null),
      row("context", "e", 5, 4),
      row("added", "f", null, 5),
      row("added", "g", null, 6),
    ];
    expect(diffBlocks(rows)).toEqual([
      { kind: "changed", from: 2, to: 2, removed: ["b"] },
      { kind: "removed", from: 4, to: 3, removed: ["d"] },
      { kind: "added", from: 5, to: 6, removed: [] },
    ]);
  });

  it("puts lines removed from the top before line 1", () => {
    const rows = [row("removed", "x", 1, null), row("context", "a", 2, 1)];
    expect(diffBlocks(rows)).toEqual([{ kind: "removed", from: 1, to: 0, removed: ["x"] }]);
  });
});

describe("lineMap", () => {
  it("follows lines past an insertion and drops edited ones", () => {
    const map = lineMap(["a", "b", "c", "d"], ["a", "new", "b", "C", "d"]);
    expect(map.slice(1)).toEqual([1, 3, undefined, 5]);
  });
});

describe("mapBlocks", () => {
  it("moves blocks with their lines and drops one whose lines were edited since", () => {
    const blocks = [
      { kind: "added" as const, from: 2, to: 3, removed: [] },
      { kind: "removed" as const, from: 5, to: 4, removed: ["x"] },
      { kind: "changed" as const, from: 6, to: 6, removed: ["y"] },
    ];
    // Two lines typed above everything; line 6 edited.
    const map = [undefined, 3, 4, 5, 6, 7, undefined];
    expect(mapBlocks(blocks, map, 9)).toEqual([
      { kind: "added", from: 4, to: 5, removed: [] },
      { kind: "removed", from: 7, to: 6, removed: ["x"] },
    ]);
  });
});
