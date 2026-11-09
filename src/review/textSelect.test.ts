import { describe, expect, it } from "vitest";
import { Keys } from "../editor/modal/keys";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, flatten } from "./model";
import { lineEdge, lineTexts, selectedOn, selectedText, startBuffer } from "./textSelect";

const files = flatten(
  review([file("a.rs", [rows([" fn a() {", "-  old();", "+  new();", " }"])])]),
);

const view = { height: 20, hidden: [] };

describe("review text select", () => {
  it("shows the new side in split view and code text on code lines only", () => {
    const unified = lineTexts(files, buildLines(files, layout()), "unified").split("\n");
    expect(unified).toEqual(["", "", "fn a() {", "  old();", "  new();", "}"]);
    const split = lineTexts(files, buildLines(files, layout({ mode: "split" })), "split");
    expect(split.split("\n")).toEqual(["", "", "fn a() {", "  new();", "}"]);
  });

  it("starts on the line's first non-blank and selects with motions, read-only", () => {
    const lines = buildLines(files, layout());
    const b = startBuffer(lineTexts(files, lines, "unified"), 4);
    const keys = new Keys(true, "select");
    keys.press(b, { key: "e" }, view);
    expect(selectedText(b)).toBe("new");
    keys.press(b, { key: "d" }, view);
    expect(b.text).toContain("new();");
    expect(selectedOn(b, 4)).toEqual({ from: 2, to: 5, head: 4 });
    expect(selectedOn(b, 3)).toBeUndefined();
  });

  it("spans lines and ends at the edges with Home and End", () => {
    const lines = buildLines(files, layout());
    const b = startBuffer(lineTexts(files, lines, "unified"), 3);
    const keys = new Keys(true, "select");
    keys.press(b, { key: "j" }, view);
    expect(selectedText(b)).toBe("old();\n  n");
    lineEdge(b, true);
    expect(selectedOn(b, 4)).toEqual({ from: 7, to: 8, head: 7 });
    lineEdge(b, false);
    expect(b.sel.head).toBe(b.lineStart(4));
  });
});
