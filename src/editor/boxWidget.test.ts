import { EditorSelection, EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { selectedLines } from "./boxWidget";

const doc = "one\ntwo\nthree\nfour";
const at = (anchor: number, head = anchor) =>
  EditorState.create({ doc, selection: EditorSelection.single(anchor, head) });

describe("selectedLines", () => {
  it("stops on the line before when the selection ends at a line's start", () => {
    // From the start of "two" to the start of "four".
    expect(selectedLines(at(4, 14))).toEqual({ start: 2, end: 3, text: "two\nthree\n", pos: 13 });
  });

  it("takes the cursor's whole line when nothing is selected", () => {
    expect(selectedLines(at(9))).toEqual({ start: 3, end: 3, text: "three", pos: 13 });
  });
});
