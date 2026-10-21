// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { textRanges } from "./domFind";

describe("textRanges", () => {
  it("finds matches that elements split, ignoring case", () => {
    const root = document.createElement("div");
    root.innerHTML = "<p>Find <b>the wo</b>rd, then the WORD</p>";
    const found = textRanges(root, "word", false);
    expect(found.map((r) => r.toString())).toEqual(["word", "WORD"]);
    expect(found[0].startContainer.textContent).toBe("the wo");
  });
});
