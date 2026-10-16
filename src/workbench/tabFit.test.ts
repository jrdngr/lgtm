import { describe, expect, it } from "vitest";
import { titleCap } from "./tabFit";

describe("titleCap", () => {
  it("leaves titles whole while they fit", () => {
    expect(titleCap([50, 120, 80], 250)).toBeUndefined();
    expect(titleCap([], 0)).toBeUndefined();
  });

  it("shortens the longest titles first and keeps short ones whole", () => {
    // 50 stays; 180 left for the other two.
    expect(titleCap([50, 300, 120], 230)).toBe(90);
    // Both long titles over the cap: 200 split two ways.
    expect(titleCap([40, 300, 400], 240)).toBe(100);
  });

  it("caps every title equally when even the shortest won't fit", () => {
    expect(titleCap([100, 100, 100], 150)).toBe(50);
    expect(titleCap([100], -10)).toBe(0);
  });
});
