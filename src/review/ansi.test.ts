import { describe, expect, it } from "vitest";
import { parseAnsi } from "./ansi";

describe("parseAnsi", () => {
  it("applies bold, basic and 256 colors, and resets", () => {
    expect(parseAnsi("\x1b[1m\x1b[38;5;2m@\x1b[0m  a\x1b[31mb\x1b[39mc")).toEqual([
      { text: "@", bold: true, color: "#3fb950" },
      { text: "  a" },
      { text: "b", color: "#f85149" },
      { text: "c", color: undefined },
    ]);
  });
});
