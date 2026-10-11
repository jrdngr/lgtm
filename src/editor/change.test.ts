import { describe, expect, it } from "vitest";
import { minimalChange } from "./change";

const apply = (s: string, c: ReturnType<typeof minimalChange>) =>
  s.slice(0, c.from) + c.insert + s.slice(c.to);

describe("minimalChange", () => {
  it("replaces only the middle that differs", () => {
    const c = minimalChange("a\nb\nc\n", "a\nB\nc\n");
    expect(c).toEqual({ from: 2, to: 3, insert: "B" });
  });

  it("handles repeated text at the seam without overlapping", () => {
    for (const [a, b] of [
      ["aaa", "aa"],
      ["aa", "aaaa"],
      ["", "x"],
      ["x", ""],
      ["abcabc", "abc"],
    ]) {
      expect(apply(a, minimalChange(a, b))).toBe(b);
    }
  });
});
