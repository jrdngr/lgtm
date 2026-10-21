import { describe, expect, it } from "vitest";
import type { CanvasGroup } from "../bindings";
import { ungroup } from "./canvasGroups";
import {
  dragBox,
  newShape,
  removeSelected,
  resized,
  SHAPE_H,
  SHAPE_MIN,
  SHAPE_W,
  setTag,
} from "./edits";
import { groupRef } from "./refs";
import { fullCanvas, type View, viewOf } from "./view";

const empty = (): View => viewOf(fullCanvas(undefined), "main");
const frame = (members: string[]): CanvasGroup => ({
  id: "f",
  label: "Frame",
  color: null,
  members,
  by: "user",
});

describe("setTag", () => {
  it("moves a digit to its new element, and an element keeps only its newest digit", () => {
    let v = setTag(empty(), "1", "s:a");
    v = setTag(v, "2", "s:a");
    v = setTag(v, "2", "s:b");
    expect(v.tags).toEqual({ "2": "s:b" });
  });

  it("with toggle, takes a tag off the element it's already on", () => {
    const v = setTag(setTag(empty(), "3", "c:api"), "3", "c:api", true);
    expect(v.tags).toEqual({});
  });
});

describe("removeSelected", () => {
  it("drops a removed shape from its frame and its tag, keeping parts and their tags", () => {
    const v: View = {
      ...empty(),
      shapes: [newShape("s:a", "rect", { x: 0, y: 0, w: 10, h: 10 })],
      groups: [frame(["s:a", "c:api"])],
      tags: { "1": "s:a", "2": "c:api" },
    };
    const out = removeSelected(v, new Set(["s:a", "c:api"]), (x) => x);
    expect(out.shapes).toEqual([]);
    expect(out.groups[0].members).toEqual(["c:api"]);
    expect(out.tags).toEqual({ "2": "c:api" });
  });

  it("ungroups a removed frame and drops the tag on it, keeping its members", () => {
    const v: View = { ...empty(), groups: [frame(["c:api"])], tags: { "4": "f" } };
    const out = removeSelected(v, new Set([groupRef("f")]), (x) => ungroup(x, "f"));
    expect(out.groups).toEqual([]);
    expect(out.tags).toEqual({});
  });
});

describe("dragBox", () => {
  it("draws the same box whichever way the drag went", () => {
    expect(dragBox({ x: 100, y: 80 }, { x: 20, y: 10 }, true)).toEqual({
      x: 20,
      y: 10,
      w: 80,
      h: 70,
    });
  });

  it("gives a click the default size at the click", () => {
    expect(dragBox({ x: 5, y: 6 }, { x: 5, y: 6 }, false)).toEqual({
      x: 5,
      y: 6,
      w: SHAPE_W,
      h: SHAPE_H,
    });
  });
});

describe("resized", () => {
  const from = { x: 100, y: 100, w: 80, h: 60 };

  it("keeps the opposite corner put", () => {
    expect(resized(from, 0, -20, -10)).toEqual({ x: 80, y: 90, w: 100, h: 70 });
    expect(resized(from, 3, 20, 10)).toEqual({ x: 100, y: 100, w: 100, h: 70 });
  });

  it("stops at the smallest size when dragged past the opposite corner", () => {
    expect(resized(from, 0, 200, 200)).toEqual({ x: 160, y: 140, w: SHAPE_MIN, h: SHAPE_MIN });
  });
});
