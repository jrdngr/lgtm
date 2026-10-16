import { describe, expect, test } from "vitest";
import { edgeAt, styledPaths } from "./paths";
import type { SceneEdge } from "./scene";

const e = (key: string, style: SceneEdge["style"]): SceneEdge => ({
  key,
  from: key.split(">")[0],
  to: key.split(">")[1],
  style,
});

const routes = new Map([
  [
    "a>b",
    [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
    ],
  ],
  [
    "a>c",
    [
      { x: 0, y: 0 },
      { x: 50, y: 0 },
      { x: 50, y: 100 },
    ],
  ],
  [
    "b>c",
    [
      { x: 100, y: 0 },
      { x: 100, y: 100 },
    ],
  ],
]);

describe("styled paths", () => {
  const edges = [e("a>b", "add"), e("a>c", "add"), e("b>c", "wrong")];

  test("edges of one style share one path, with an arrowhead each", () => {
    const { lines } = styledPaths(edges, routes);
    expect([...lines.keys()].sort()).toEqual(["edge add", "edge wrong"]);
    const add = lines.get("edge add");
    expect(add?.d.match(/M/g)).toHaveLength(2);
    expect(add?.arrows.match(/z/g)).toHaveLength(2);
  });

  test("a focus splits styles into the edges in it and the dimmed rest", () => {
    const focus = { nodes: new Set(["a", "b"]), edges: new Set(["a>b"]) };
    const { lines } = styledPaths(edges, routes, focus);
    expect([...lines.keys()].sort()).toEqual(["edge add dim", "edge add hi", "edge wrong dim"]);
  });

  test("a Wrong edge gets its badge halfway along", () => {
    expect(styledPaths(edges, routes).badges).toEqual([{ x: 100, y: 50 }]);
  });
});

describe("finding an edge under the pointer", () => {
  const edges = [e("a>b", "ctx"), e("a>c", "ctx"), e("b>c", "ctx")];

  test("picks the nearest edge within reach", () => {
    expect(edgeAt(edges, routes, { x: 53, y: 60 }, 6)).toBe("a>c");
    expect(edgeAt(edges, routes, { x: 97, y: 60 }, 6)).toBe("b>c");
  });

  test("finds none when the pointer is out of reach", () => {
    expect(edgeAt(edges, routes, { x: 75, y: 60 }, 6)).toBeUndefined();
  });
});
