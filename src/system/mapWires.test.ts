import { describe, expect, it } from "vitest";
import type { Layout, Placed } from "./layout";
import { focusWires, mapFocus, restWires } from "./mapWires";
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
      over: true,
      pill: { kind: "label", text: "calls" },
    });
    expect(w.find((x) => x.key === "c>d")).toMatchObject({
      faint: true,
      over: false,
      pill: undefined,
    });
    expect(w.find((x) => x.key === "c>a")).toMatchObject({ faint: false, tone: "removed" });
  });

  it("keeps quiet changed edges hairlines until lit, then colored with their label", () => {
    const quiet = (hovered: string[]) =>
      restWires(layout, edges, { hovered: new Set(hovered), hot: new Map(), quiet: true });
    expect(quiet([]).map((x) => x.pill)).toEqual([undefined, undefined, undefined, undefined]);
    expect(quiet([]).map((x) => x.tone)).toEqual([undefined, undefined, undefined, undefined]);
    const lit = edges.map((e) => (e.from === "c" && e.to === "d" ? { ...e, label: "feeds" } : e));
    const w = restWires(layout, lit, { hovered: new Set(["c>d"]), hot: new Map(), quiet: true });
    expect(w.find((x) => x.key === "c>d")).toMatchObject({
      tone: "added",
      pill: { kind: "label", text: "feeds" },
    });
  });

  it("slides lit labels along their wires apart when they'd overlap", () => {
    const close: Layout = {
      ...layout,
      nodes: [node("a", 0, 0), node("b", 300, 0), node("c", 0, 6), node("d", 300, 6)],
    };
    const two: SEdge[] = [
      { from: "a", to: "b", imports: 1, label: "verifies tokens" },
      { from: "c", to: "d", imports: 1, label: "watches" },
    ];
    const [x, y] = restWires(close, two, { hovered: new Set(["a>b", "c>d"]), hot: new Map() });
    const apart = Math.abs(x.mid.x - y.mid.x) > 85 || Math.abs(x.mid.y - y.mid.y) >= 18;
    expect(apart).toBe(true);
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

describe("focusWires", () => {
  const all: SEdge[] = [
    { from: "a", to: "b", imports: 1, label: "Reads" },
    { from: "b", to: "a", imports: 1, label: "Notifies" },
    { from: "b", to: "c", imports: 1, label: "Writes" },
    { from: "c", to: "d", imports: 1, label: "Hidden" },
  ];
  const wires = (hover: { node?: string; edge?: string } = {}) =>
    focusWires(mapFocus(all, "b"), "b", all, new Set(), hover, undefined);

  it("labels each neighbor once, by the neighbor, both ways joined", () => {
    const w = wires();
    const f = mapFocus(all, "b");
    const a = f.pos.get("a");
    const c = f.pos.get("c");
    if (!a || !c) throw new Error("missing");
    expect(w.map((x) => [x.key, x.pill?.text, x.anchor])).toEqual([
      ["a>b", "Reads · Notifies", "start"],
      ["b>a", undefined, "start"],
      ["b>c", "Writes", "end"],
    ]);
    expect(w[0].mid.x).toBeGreaterThan(a.x + a.w);
    expect(w[2].mid.x).toBeLessThan(c.x);
    expect(w[0].mid.y).toBe(a.y + a.h / 2);
  });

  it("fades every wire but the hovered neighbor's or edge's", () => {
    expect(wires({ node: "c" }).map((x) => [x.key, x.faint, x.alone])).toEqual([
      ["a>b", true, false],
      ["b>a", true, false],
      ["b>c", false, true],
    ]);
    expect(wires({ edge: "b>a" }).map((x) => x.faint)).toEqual([true, false, true]);
  });
});
