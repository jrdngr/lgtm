import { describe, expect, it } from "vitest";
import { DEFAULT_SLOTS, markSetup } from "./marks";
import { progressParts } from "./state";

describe("progressParts", () => {
  it("lists marks in setup order with their done flag, then file-done and unmarked rows", () => {
    const setup = markSetup(DEFAULT_SLOTS);
    const parts = progressParts(
      { marks: { gray: 214, green: 812, red: 96 }, byFile: 258, done: 1284, changed: 1780 },
      setup,
    );
    expect(parts.map((x) => [x.label, x.rows, x.done])).toEqual([
      ["reviewed", 812, true],
      ["flagged", 96, false],
      ["skipped", 214, true],
      ["checked files", 258, true],
      ["unmarked", 400, false],
    ]);
  });

  it("leaves out parts with no rows", () => {
    const parts = progressParts(
      { marks: {}, byFile: 0, done: 0, changed: 10 },
      markSetup(DEFAULT_SLOTS),
    );
    expect(parts.map((x) => x.key)).toEqual(["unmarked"]);
  });
});
