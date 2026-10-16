// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { lineRef, renderedLines } from "./renderedLines";

function rendered() {
  const root = document.createElement("div");
  root.innerHTML =
    '<h1 data-start="1" data-end="1">Title</h1>' +
    '<p data-start="3" data-end="4">One <b>two</b></p>' +
    '<ul><li data-start="6" data-end="6">Item</li></ul>';
  document.body.replaceChildren(root);
  return root;
}

describe("renderedLines", () => {
  it("spans the source lines of every block the selection touches", () => {
    const root = rendered();
    const sel = document.getSelection();
    sel?.setBaseAndExtent(
      root.querySelector("b")?.firstChild as Node,
      1,
      root.querySelector("li")?.firstChild as Node,
      2,
    );
    expect(renderedLines(root)).toEqual([3, 6]);
  });

  it("falls back to the block at the click when nothing is selected", () => {
    const root = rendered();
    document.getSelection()?.removeAllRanges();
    expect(renderedLines(root, root.querySelector("h1")?.firstChild ?? undefined)).toEqual([1, 1]);
    expect(renderedLines(root, root.querySelector("ul") ?? undefined)).toBeUndefined();
  });
});

describe("lineRef", () => {
  it("names one line or a range", () => {
    expect(lineRef("a.md", [2, 2])).toBe("a.md:2");
    expect(lineRef("a.md", [2, 5])).toBe("a.md:2-5");
  });
});
