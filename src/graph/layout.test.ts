import { expect, test } from "vitest";
import type { GraphDto, GraphNode } from "../bindings";
import { layout } from "./layout";

const node = (id: number, name: string, path: string, status: GraphNode["status"]): GraphNode => ({
  id,
  name,
  kind: "function",
  status,
  path,
  file: status === "caller" ? null : 0,
  line: 1,
  rows: status === "caller" ? [] : [0],
  parent: null,
});

test("lays out symbols inside their file's box, callers left of what they call", async () => {
  const graph: GraphDto = {
    nodes: [node(0, "helper", "a.rs", "modified"), node(1, "caller", "b.rs", "caller")],
    edges: [{ from: 1, to: 0 }],
  };
  const placed = await layout(graph);
  const a = placed.groups.get("a.rs");
  const b = placed.groups.get("b.rs");
  const helper = placed.nodes.get(0);
  expect(a && b && helper).toBeTruthy();
  if (!a || !b || !helper) return;
  expect(b.x + b.width).toBeLessThanOrEqual(a.x);
  // Inside its box, in the box's coordinates.
  expect(helper.x + helper.width).toBeLessThanOrEqual(a.width);
  expect(helper.y + helper.height).toBeLessThanOrEqual(a.height);
});
