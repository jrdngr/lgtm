import { describe, expect, it } from "vitest";
import type { Layout, Placed } from "./layout";
import { mapFocus, restWires } from "./mapWires";
import type { SEdge } from "./scene";

const node = (id: string, x: number, y: number): Placed => ({
  id,
  kind: "component",
  name: id,
  x,
  y,
  w: 100,
  h: 40,
});

const layout: Layout = {
  nodes: [node("a", 0, 0), node("b", 300, 0), node("c", 0, 200), node("d", 300, 200)],
  tiers: [],
  width: 400,
  height: 240,
};

const edges: SEdge[] = [
  { from: "a", to: "b", imports: 0, label: "calls" },
  { from: "b", to: "d", imports: 3 },
  { from: "c", to: "d", imports: 2, status: "added" },
  { from: "c", to: "a", imports: 1, status: "removed" },
];

const rest = (hovered: string[] = [], hot = new Map<string, number | undefined>()) =>
  restWires(layout, edges, { hovered: new Set(hovered), hot });
const bright = (w: ReturnType<typeof rest>) => w.filter((x) => x.tone).map((x) => x.key);

describe("restWires", () => {
  it("draws only the review's edges bright at rest, each with its status pill", () => {
    const w = rest();
    expect(bright(w)).toEqual(["c>d", "c>a"]);
    expect(w.map((x) => x.pill?.kind)).toEqual([undefined, undefined, "added", "removed"]);
  });

  it("lights a hovered node's edges with their labels and fades the other changed edges", () => {
    const w = rest(["a>b", "c>a"]);
    expect(bright(w)).toEqual(["a>b", "c>d", "c>a"]);
    expect(w.find((x) => x.key === "a>b")).toMatchObject({
      tone: "blue",
      pill: { kind: "label", text: "calls" },
    });
    expect(w.find((x) => x.key === "c>d")).toMatchObject({ faint: true, pill: undefined });
    expect(w.find((x) => x.key === "c>a")).toMatchObject({ faint: false, tone: "removed" });
  });

  it("gives an unlabeled lit edge no pill", () => {
    expect(rest(["b>d"]).find((x) => x.key === "b>d")?.pill).toBeUndefined();
  });

  it("numbers a trace's hops in the hot color", () => {
    const w = rest([], new Map([["b>d", 2]]));
    expect(w.find((x) => x.key === "b>d")).toMatchObject({
      tone: "hot",
      pill: { kind: "num", text: "2" },
    });
  });
});

describe("mapFocus", () => {
  it("puts users left and uses right, a two-way neighbor left only, and drops other edges", () => {
    const f = mapFocus(
      [...edges, { from: "b", to: "c", imports: 1 }, { from: "d", to: "b", imports: 1 }],
      "b",
    );
    const x = (k: string) => f.pos.get(k)?.x;
    expect(x("a")).toBe(0);
    expect(x("d")).toBe(0);
    expect(x("b")).toBeGreaterThan(0);
    expect(x("c")).toBeGreaterThan(x("b") ?? 0);
    expect(f.links).toEqual([
      ["a", "b"],
      ["b", "d"],
      ["b", "c"],
      ["d", "b"],
    ]);
  });
});
