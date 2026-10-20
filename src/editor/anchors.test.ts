import { describe, expect, it } from "vitest";
import { anchorLine } from "./anchors";

const doc = [
  "# Design: Phase 1",
  "",
  "## Usage & setup",
  "```md",
  "## Usage & setup",
  "```",
  "## Usage & setup",
  "",
  "Notes",
  "-----",
  '<a id="raw-anchor"></a>',
  "### `code` and [a link](x.md) ###",
].join("\n");

describe("anchorLine", () => {
  it("finds headings by GitHub id, numbering repeats and skipping fenced code", () => {
    expect(anchorLine(doc, "design-phase-1")).toBe(1);
    expect(anchorLine(doc, "usage--setup")).toBe(3);
    expect(anchorLine(doc, "usage--setup-1")).toBe(7);
    expect(anchorLine(doc, "notes")).toBe(9);
    expect(anchorLine(doc, "code-and-a-link")).toBe(12);
  });

  it("finds html ids and GitHub line links, and nothing for unknown anchors", () => {
    expect(anchorLine(doc, "raw-anchor")).toBe(11);
    expect(anchorLine(doc, "L9-L10")).toBe(9);
    expect(anchorLine(doc, "missing")).toBeUndefined();
  });
});
