import { describe, expect, it } from "vitest";
import { Buffer } from "./buffer";
import { Folds, indentSpans, ranges } from "./folds";

describe("folds", () => {
  it("folds a file without a grammar by indent", () => {
    const spans = indentSpans(new Buffer("a\n  b\n\n  c\n    d\ne\n", "t.txt"));
    expect([...spans]).toEqual([
      [0, 4],
      [3, 4],
    ]);
  });

  it("toggles, closes everything and reveals a line", () => {
    const f = new Folds();
    f.update(ranges(indentSpans(new Buffer("a\n  b\n  c\n    d\ne\n", "t.txt"))));
    f.apply("toggle", 0);
    expect(f.hidden()).toEqual([[0, 3]]);
    f.apply("toggle", 0);
    expect(f.hidden()).toEqual([]);
    f.apply("closeAll", 0);
    expect(f.hidden()).toEqual([[0, 3]]);
    f.reveal(3);
    expect(f.hidden()).toEqual([]);
  });
});
