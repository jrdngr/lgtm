import { describe, expect, it } from "vitest";
import type { Layout, Placed } from "../system/layout";
import { arrange, freeze } from "./arrange";
import { fullCanvas, type View, viewOf } from "./view";

const node = (id: string, x: number, y: number): Placed => ({
  id,
  kind: "container",
  name: id,
  x,
  y,
  w: 200,
  h: 100,
});

const base: Layout = {
  nodes: [node("c:a", 40, 40), node("c:b", 300, 40), node("c:c", 40, 200)],
  tiers: [{ label: "Apps", x: 0, y: 0, w: 600, h: 160 }],
  boundary: { name: "sys", x: 4, y: 4, w: 560, h: 340 },
  width: 600,
  height: 400,
};
const edges = [{ from: "c:a", to: "c:c", imports: 1 }];
const empty = (): View => viewOf(fullCanvas(undefined), "containers");
const at = (l: Layout, id: string) => l.nodes.find((n) => n.id === id);

describe("arrange", () => {
  it("keeps the automatic layout for a view never arranged", () => {
    const a = arrange(base, edges, empty(), {});
    expect(a.layout.nodes.map((n) => [n.x, n.y])).toEqual(base.nodes.map((n) => [n.x, n.y]));
    expect(a.layout.tiers).toEqual(base.tiers);
    expect(a.fresh.size).toBe(0);
  });

  it("places a part new since the freeze in open space near what it links to", () => {
    const v = freeze(empty(), base);
    delete v.positions["c:c"];
    v.positions["c:b"] = { x: 600, y: 600 };
    const a = arrange(base, edges, v, {});
    const c = at(a.layout, "c:c");
    const others = a.layout.nodes.filter((n) => n.id !== "c:c");
    expect(c).toBeDefined();
    if (!c) return;
    for (const o of others)
      expect(c.x >= o.x + o.w || o.x >= c.x + c.w || c.y >= o.y + o.h || o.y >= c.y + c.h).toBe(
        true,
      );
    expect(Math.abs(c.x - 40)).toBeLessThan(200);
    expect([...a.fresh]).toEqual(["c:c"]);
  });

  it("drops the tier bands and fits the boundary to the parts it held once moved", () => {
    const v = freeze(empty(), base);
    v.positions["c:b"] = { x: 900, y: 40 };
    const a = arrange(base, edges, v, {});
    expect(a.layout.tiers).toEqual([]);
    const b = a.layout.boundary;
    expect(b && b.x + b.w).toBeGreaterThan(900 + 200);
    expect(a.layout.width).toBeGreaterThan(1100);
  });

  it("moves only the dragged parts, never above or left of the world", () => {
    const a = arrange(base, edges, empty(), {}, { ids: new Set(["c:a"]), dx: -100, dy: 30 });
    expect(at(a.layout, "c:a")).toMatchObject({ x: 0, y: 70 });
    expect(at(a.layout, "c:b")).toMatchObject({ x: 300, y: 40 });
  });

  it("tints a colored part", () => {
    const a = arrange(base, edges, empty(), { "c:b": "red" });
    expect(at(a.layout, "c:b")?.paint).toBe("#e5534b");
    expect(at(a.layout, "c:a")?.paint).toBeUndefined();
  });
});
