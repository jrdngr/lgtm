import { describe, expect, it } from "vitest";
import { coveredCount, pathChoices } from "./paths";

const ALL = ["docs/design/a.md", "docs/design/b/c.md", "docs/designs.md", "src/main.rs"];

describe("coveredCount", () => {
  it("counts the files under a folder, not ones that only share its prefix", () => {
    expect(coveredCount(ALL, "docs/design/")).toBe(2);
    expect(coveredCount(ALL, "src/main.rs")).toBe(1);
  });
});

describe("pathChoices", () => {
  it("lists matching folders shallow first, then files, without taken ones", () => {
    expect(pathChoices(ALL, "design", ["docs/design/a.md"])).toEqual([
      "docs/design/",
      "docs/design/b/",
      "docs/design/b/c.md",
      "docs/designs.md",
    ]);
  });

  it("lists nothing until something is typed", () => {
    expect(pathChoices(ALL, "  ", [])).toEqual([]);
  });
});
