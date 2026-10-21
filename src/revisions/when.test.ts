import { describe, expect, it } from "vitest";
import { when } from "./when";

describe("when", () => {
  const now = new Date(2026, 9, 21, 15, 0).getTime();
  const at = (d: Date) => d.getTime() / 1000;

  it("says how long ago for the last day, then Yesterday, then the date", () => {
    expect(when(at(new Date(2026, 9, 21, 14, 48)), now)).toBe("12m");
    expect(when(at(new Date(2026, 9, 20, 9, 0)), now)).toBe("Yesterday");
    expect(when(at(new Date(2026, 9, 18, 9, 0)), now)).toBe("Oct 18");
  });

  it("is empty for a commit with no time, like the root", () => {
    expect(when(0, now)).toBe("");
  });
});
