import { describe, expect, it } from "vitest";
import { matchOffsets, needleOf } from "./selectionMatches";

describe("matchOffsets", () => {
  it("matches a word only where it stands alone", () => {
    expect(matchOffsets("id = idle(id_x, $id) + id", "id")).toEqual([0, 23]);
  });

  it("matches other text anywhere, case-sensitively", () => {
    expect(matchOffsets("a.b(x) A.B(x) a.b(", "a.b(")).toEqual([0, 14]);
  });

  it("doesn't count overlapping matches twice", () => {
    expect(matchOffsets("====", "==")).toEqual([0, 2]);
  });
});

describe("needleOf", () => {
  it("skips selections under 2 characters or spanning lines", () => {
    expect(needleOf("x")).toBeUndefined();
    expect(needleOf("a\nb")).toBeUndefined();
    expect(needleOf("ab")).toBe("ab");
  });
});
