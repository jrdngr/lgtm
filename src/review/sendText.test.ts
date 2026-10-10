import { describe, expect, it } from "vitest";
import type { ReviewDto } from "../bindings";
import { file, rows } from "./fixture.test-utils";
import { type DisplayLine, flatten } from "./model";
import { selectionText } from "./sendText";

const review = {
  files: [
    file("a.rs", [rows([" fn a() {", "-  old();", "+  new();", " }"]), rows(["-gone"], 9, 9)]),
  ],
} as unknown as ReviewDto;
const files = flatten(review);
const line = (r: number[], hunk = 0): DisplayLine => ({ kind: "rows", file: 0, hunk, rows: r });

describe("selectionText", () => {
  it("writes each hunk's rows as a diff under the path and line range", () => {
    const lines = [
      { kind: "file", file: 0 } as const,
      line([0]),
      line([1]),
      line([2]),
      line([4], 1),
    ];
    expect(selectionText(files, lines, 1, 4)).toBe(
      "a.rs:1-2\n```diff\n fn a() {\n-  old();\n+  new();\n```\n\n" +
        "a.rs:9 (removed)\n```diff\n-gone\n```",
    );
  });

  it("orders split-view pairs as a unified diff", () => {
    expect(selectionText(files, [line([1, 2])], 0, 0)).toBe(
      "a.rs:2\n```diff\n-  old();\n+  new();\n```",
    );
  });
});
