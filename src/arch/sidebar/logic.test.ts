import { describe, expect, test } from "vitest";
import type { HistoryDto, LayerDto, MarkColor } from "../../bindings";
import { rows } from "../../review/fixture.test-utils";
import type { RowState } from "../../review/rowState";
import { clickRange, layerDone, meterTone, pickedCommits, rowsInLines } from "./logic";

test("meter is red to 2, amber at 3, green from 4", () => {
  expect([1, 2, 3, 4, 5].map(meterTone)).toEqual(["lo", "lo", "mid", "hi", "hi"]);
});

describe("rowsInLines", () => {
  test("a removed row belongs to lines around it, not to the line after", () => {
    const r = rows([" a", "-gone", " b", " c"]);
    expect(rowsInLines(r, 1, 2)).toEqual([0, 1, 2]);
    expect(rowsInLines(r, 2, 3)).toEqual([2, 3]);
  });

  test("a removed file uses old lines", () => {
    expect(rowsInLines(rows(["-a", "-b", "-c"]), 2, 3)).toEqual([1, 2]);
  });
});

describe("layerDone", () => {
  const files = [{ rows: rows([" a", "+b", "+c", "+d", " e"]) }, { rows: rows(["+x"]) }];
  const marked = new Map<string, RowState>([
    ["0:1", "checked"],
    ["0:2", "cyan"],
  ]);
  const review = (done: (c: MarkColor) => boolean) => ({
    files,
    rowState: (f: number, r: number) => marked.get(`${f}:${r}`) ?? "unmarked",
    done,
  });
  const layer = (ranges: [number | null, number, number][]): LayerDto => ({
    n: 1,
    title: "",
    summary: "",
    components: [],
    ranges: ranges.map(([file, start, end]) => ({ path: "", file, start, end })),
  });

  const none = review(() => false);
  const cyan = review((c) => c === "cyan");
  const all = review(() => true);

  test("counts changed rows once across overlapping ranges, by the marks that mean done", () => {
    const l = layer([
      [0, 1, 3],
      [0, 2, 5],
    ]);
    expect(layerDone(none, l)).toBeCloseTo(1 / 3);
    expect(layerDone(cyan, l)).toBeCloseTo(2 / 3);
  });

  test("spans files, and skips ranges outside the review", () => {
    const l = layer([
      [0, 2, 3],
      [1, 1, 1],
      [null, 1, 9],
    ]);
    expect(layerDone(all, l)).toBeCloseTo(2 / 3);
  });

  test("is undefined when no row needs review", () => {
    expect(layerDone(all, layer([[0, 1, 1]]))).toBeUndefined();
  });
});

describe("history clicks", () => {
  const commit = (id: string, parent: string | null): HistoryDto => ({
    changeId: id,
    commitId: id,
    parent,
    description: "",
    componentsAdded: 0,
    componentsRemoved: 0,
    edgesAdded: 0,
    edgesRemoved: 0,
  });
  const list = [commit("a", "base"), commit("b", "a"), commit("c", "b")];
  const [a, b, c] = list;

  test("a click shows the commit's own delta, and a second click the whole review", () => {
    const one = clickRange(list, undefined, b, false);
    expect(one).toEqual({ from: "a", to: "b" });
    expect(pickedCommits(list, one)).toEqual(["b"]);
    expect(clickRange(list, one, b, false)).toBeUndefined();
  });

  test("shift-click compares older to newer, whichever is clicked first", () => {
    const shown = clickRange(list, undefined, c, false);
    const both = clickRange(list, shown, a, true);
    expect(both).toEqual({ from: "a", to: "c" });
    expect(pickedCommits(list, both)).toEqual(["a", "c"]);
  });

  test("shift-click with nothing or a comparison shown picks one commit", () => {
    expect(clickRange(list, undefined, a, true)).toEqual({ from: "base", to: "a" });
    expect(clickRange(list, { from: "a", to: "c" }, b, true)).toEqual({ from: "a", to: "b" });
  });
});
