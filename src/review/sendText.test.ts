import { describe, expect, it } from "vitest";
import { file, review, rows } from "./fixture.test-utils";
import { type DisplayLine, flatten } from "./model";
import { locatedText, selectionText } from "./sendText";

const files = flatten(
  review([
    file("a.rs", [rows([" fn a() {", "-  old();", "+  new();", " }"]), rows(["-gone"], 9, 9)]),
  ]),
);
const line = (r: number[], hunk = 0): DisplayLine => ({ kind: "rows", file: 0, hunk, rows: r });

describe("selectionText", () => {
  it("names each hunk's path and line ranges, with removed rows' old range", () => {
    const lines = [
      { kind: "file", file: 0 } as const,
      line([0]),
      line([1]),
      line([2]),
      line([4], 1),
    ];
    expect(selectionText(files, lines, 1, 4)).toBe(
      "a.rs:1-2, and removed old line 2\na.rs:9 (removed, old version)",
    );
  });

  it("leaves out removals when every row is on the new side", () => {
    expect(selectionText(files, [line([0]), line([2])], 0, 1)).toBe("a.rs:1-2");
  });
});

describe("locatedText", () => {
  it("puts the rows under their new side's range across hunks", () => {
    const lines = [line([0]), line([1]), line([2]), line([4], 1)];
    expect(locatedText(files, lines, 0, 3)).toBe("a.rs:1-2\nfn a() {\n  old();\n  new();\ngone");
  });
});
