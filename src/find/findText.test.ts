import { describe, expect, it } from "vitest";
import { findAll, stepMatch } from "./findText";

describe("findAll", () => {
  it("ignores case unless asked, and doesn't overlap", () => {
    expect(findAll("Foo foo FOO", "foo", false)).toEqual([0, 4, 8]);
    expect(findAll("Foo foo FOO", "foo", true)).toEqual([4]);
    expect(findAll("aaaa", "aa", false)).toEqual([0, 2]);
    expect(findAll("abc", "", false)).toEqual([]);
  });
});

describe("stepMatch", () => {
  it("wraps both ways", () => {
    expect(stepMatch(2, 3, false)).toBe(0);
    expect(stepMatch(0, 3, true)).toBe(2);
    expect(stepMatch(0, 0, false)).toBe(0);
  });
});
