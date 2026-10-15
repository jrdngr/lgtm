import { describe, expect, it } from "vitest";
import type { RowDto } from "../bindings";
import { snippetLines, unified } from "./snippet";

const row = (kind: RowDto["kind"], oldLine: number | null, newLine: number | null, text: string) =>
  ({ kind, oldLine, newLine, text }) as RowDto;

describe("snippet", () => {
  it("writes a hunk per run of lines and numbers them back the same", () => {
    const text = unified([
      row("context", 4, 4, "a"),
      row("removed", 5, null, "b"),
      row("added", null, 5, "B"),
      row("context", 20, 21, "z"),
    ]);
    expect(text).toBe("@@ -4,2 +4,2 @@\n a\n-b\n+B\n@@ -20,1 +21,1 @@\n z");
    expect(snippetLines(text).map((l) => [l.kind, l.old, l.new, l.text])).toEqual([
      ["hunk", undefined, undefined, "@@ -4,2 +4,2 @@"],
      ["context", 4, 4, "a"],
      ["removed", 5, undefined, "b"],
      ["added", undefined, 5, "B"],
      ["hunk", undefined, undefined, "@@ -20,1 +21,1 @@"],
      ["context", 20, 21, "z"],
    ]);
  });

  it("numbers GitHub's diff hunk and skips the fences of older snippets", () => {
    expect(
      snippetLines("@@ -10,2 +12,3 @@ fn x() {\n ctx\n+new").map((l) => [l.old, l.new]),
    ).toEqual([
      [undefined, undefined],
      [10, 12],
      [undefined, 13],
    ]);
    expect(snippetLines("a.rs:3\n```diff\n+x\n```")).toEqual([{ kind: "added", text: "x" }]);
  });
});
