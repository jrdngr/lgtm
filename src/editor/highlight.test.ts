import { Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { HIGHLIGHT_NAMES } from "../bindings";
import { decorate } from "./highlight";

const keyword = HIGHLIGHT_NAMES.indexOf("keyword");

function ranges(doc: Text, lines: [number | null, number][][]) {
  const out: [number, number, string][] = [];
  decorate(doc, lines).between(0, doc.length, (from, to, d) => {
    out.push([from, to, d.spec.class]);
  });
  return out;
}

describe("decorate", () => {
  it("places each line's segments from its start, skipping unstyled ones", () => {
    const doc = Text.of(["let é = 1;", "", "fn f"]);
    const got = ranges(doc, [
      [
        [keyword, 3],
        [null, 7],
      ],
      [],
      [
        [keyword, 2],
        [null, 2],
      ],
    ]);
    expect(got).toEqual([
      [0, 3, "hl-keyword"],
      [12, 14, "hl-keyword"],
    ]);
  });

  it("clamps segments to the line and ignores lines past the end", () => {
    const doc = Text.of(["fn"]);
    expect(ranges(doc, [[[keyword, 5]], [[keyword, 1]]])).toEqual([[0, 2, "hl-keyword"]]);
  });
});
