import { describe, expect, it } from "vitest";
import { hasConflicts } from "./markers";

describe("hasConflicts", () => {
  it("needs a start marker with an end marker after it", () => {
    expect(
      hasConflicts("a\n<<<<<<< conflict 1 of 1\n+++++++ x\nb\n>>>>>>> conflict 1 of 1 ends\n"),
    ).toBe(true);
    expect(hasConflicts(">>>>>>> end\n<<<<<<< start\n")).toBe(false);
  });

  it("ignores runs too short or followed by other text", () => {
    expect(hasConflicts("<<<<<< six\n>>>>>> six\n")).toBe(false);
    expect(hasConflicts("<<<<<<<x\n>>>>>>>x\n")).toBe(false);
  });
});
