import { describe, expect, test } from "vitest";
import { arrange, moved } from "./pins";

describe("moved", () => {
  test("moves a pin up and down", () => {
    expect(moved(["a", "b", "c"], "c", 0)).toEqual(["c", "a", "b"]);
    expect(moved(["a", "b", "c"], "a", 2)).toEqual(["b", "c", "a"]);
  });

  test("clamps an index past either end", () => {
    expect(moved(["a", "b"], "a", 9)).toEqual(["b", "a"]);
    expect(moved(["a", "b"], "b", -3)).toEqual(["b", "a"]);
  });
});

describe("arrange", () => {
  const items = ["main", "x", "y", "z"];

  test("puts pins on top in pin order and keeps the rest in list order", () => {
    expect(arrange(items, (i) => i, ["z", "x"])).toEqual({ top: ["z", "x"], rest: ["main", "y"] });
  });

  test("skips pins whose workspace is gone", () => {
    expect(arrange(items, (i) => i, ["gone", "y"])).toEqual({
      top: ["y"],
      rest: ["main", "x", "z"],
    });
  });
});
