import { describe, expect, it } from "vitest";
import type { CanvasGroup } from "../../bindings";
import type { Box } from "../../system/layout";
import { group, groupAt, groupBoxes, moveInto, partsIn, ungroup } from "./canvasGroups";
import { fullCanvas, type View, viewOf } from "./view";

const g = (id: string, members: string[]): CanvasGroup => ({
  id,
  label: id,
  color: null,
  members,
  by: "user",
});
const view = (groups: CanvasGroup[]): View => ({ ...viewOf(fullCanvas(undefined), "x"), groups });
const box = (x: number, y: number): Box => ({ x, y, w: 100, h: 50 });

describe("group", () => {
  it("nests a new group where its first member sat in their shared parent", () => {
    const v = group(view([g("outer", ["c:a", "c:b", "c:c"])]), ["c:b", "c:c"], "inner", "Inner");
    expect(v.groups.find((x) => x.id === "outer")?.members).toEqual(["c:a", "inner"]);
    expect(v.groups.find((x) => x.id === "inner")?.members).toEqual(["c:b", "c:c"]);
  });

  it("takes members from different parents to the top, dropping a group left empty", () => {
    const v = group(view([g("one", ["c:a"]), g("two", ["c:b", "c:c"])]), ["c:a", "c:b"], "n", "N");
    expect(v.groups.map((x) => x.id)).toEqual(["two", "n"]);
    expect(v.groups[0].members).toEqual(["c:c"]);
  });
});

describe("ungroup", () => {
  it("puts its members in its place in its parent", () => {
    const v = ungroup(
      view([g("outer", ["c:a", "inner", "c:d"]), g("inner", ["c:b", "c:c"])]),
      "inner",
    );
    expect(v.groups).toEqual([g("outer", ["c:a", "c:b", "c:c", "c:d"])]);
  });
});

describe("moveInto", () => {
  const nested = view([g("outer", ["inner", "c:x"]), g("inner", ["c:a"])]);

  it("won't put a group inside itself", () => {
    expect(moveInto(nested, ["outer"], "inner")).toBe(nested);
  });

  it("drops groups emptied by the move, all the way up", () => {
    const v = moveInto(view([g("outer", ["inner"]), g("inner", ["c:a"])]), ["c:a"], undefined);
    expect(v.groups).toEqual([]);
  });

  it("moves a part between groups", () => {
    const v = moveInto(nested, ["c:x"], "inner");
    expect(partsIn(v, "inner")).toEqual(["c:a", "c:x"]);
    expect(v.groups.find((x) => x.id === "outer")?.members).toEqual(["inner"]);
  });
});

describe("groupBoxes", () => {
  const v = view([g("outer", ["inner", "c:b"]), g("inner", ["c:a"])]);
  const parts = new Map([
    ["c:a", box(100, 100)],
    ["c:b", box(400, 100)],
  ]);

  it("frames a nested group inside its parent's frame", () => {
    const boxes = groupBoxes(v, parts);
    const inner = boxes.get("inner");
    const outer = boxes.get("outer");
    expect(inner && outer).toBeTruthy();
    if (!inner || !outer) return;
    expect(outer.x).toBeLessThan(inner.x);
    expect(outer.y).toBeLessThan(inner.y);
    expect(outer.x + outer.w).toBeGreaterThan(400 + 100);
    expect([outer.depth, inner.depth]).toEqual([0, 1]);
  });

  it("finds the innermost frame under a point, skipping the ones being dragged", () => {
    const boxes = groupBoxes(v, parts);
    expect(groupAt(boxes, 150, 120, new Set())).toBe("inner");
    expect(groupAt(boxes, 150, 120, new Set(["inner"]))).toBe("outer");
    expect(groupAt(boxes, 900, 900, new Set())).toBeUndefined();
  });

  it("leaves out a group with no members on the map", () => {
    expect(groupBoxes(view([g("gone", ["c:zz"])]), parts).has("gone")).toBe(false);
  });
});
