import { describe, expect, it } from "vitest";
import { wordDiff } from "./wordDiff";

describe("wordDiff", () => {
  it("marks the words a rewrite removed and added, keeping the rest", () => {
    expect(wordDiff("I'll loop over it directly.", "Fixed: it loops over it directly.")).toEqual([
      { kind: "del", text: "I'll loop " },
      { kind: "ins", text: "Fixed: it loops " },
      { kind: "same", text: "over it directly." },
    ]);
  });

  it("puts back the new text whole when nothing is shared", () => {
    const parts = wordDiff("old words", "new text here");
    expect(
      parts
        .filter((p) => p.kind !== "del")
        .map((p) => p.text)
        .join(""),
    ).toBe("new text here");
    expect(
      parts
        .filter((p) => p.kind !== "ins")
        .map((p) => p.text)
        .join(""),
    ).toBe("old words");
  });
});
