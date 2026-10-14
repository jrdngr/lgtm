import { describe, expect, it } from "vitest";
import { lineDiff } from "./lineDiff";

const kinds = (hunks: ReturnType<typeof lineDiff>) =>
  hunks.map((h) => h.map((l) => `${l.kind[0]}${l.line} ${l.text}`));

describe("lineDiff", () => {
  it("keeps two lines of context and splits far-apart changes into hunks", () => {
    const before = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    const after = "1\nTWO\n3\n4\n5\n6\n7\n8\n9\n10\n11\n";
    expect(kinds(lineDiff(before, after))).toEqual([
      ["c1 1", "d2 2", "a2 TWO", "c3 3", "c4 4"],
      ["c9 9", "c10 10", "a11 11"],
    ]);
  });

  it("shows a new file as all additions", () => {
    expect(kinds(lineDiff("", "a\nb\n"))).toEqual([["a1 a", "a2 b"]]);
  });
});
