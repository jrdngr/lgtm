import { describe, expect, test } from "vitest";
import type { GraphDto, GraphNode } from "../bindings";
import { layout } from "./layout";
import { buildTree } from "./tree";
import {
  firstView,
  links,
  presentBoxes,
  presentNodes,
  toggleCallers,
  type View,
  visibleBoxes,
} from "./view";

type Extra = Partial<Pick<GraphNode, "parent" | "kind" | "hop" | "line">>;

function graphOf(specs: [string, string, Extra?][], edges: [number, number][]): GraphDto {
  const nodes = specs.map(([path, name, extra], id): GraphNode => {
    const hop = extra?.hop ?? 0;
    return {
      id,
      name,
      kind: extra?.kind ?? "function",
      status: hop === 0 ? "modified" : "caller",
      path,
      file: hop === 0 ? 0 : null,
      line: extra?.line ?? id + 1,
      rows: hop === 0 ? [0] : [],
      parent: extra?.parent ?? null,
      hop,
    };
  });
  return { nodes, edges: edges.map(([from, to]) => ({ from, to })) };
}

function scene(graph: GraphDto, view: View) {
  const tree = buildTree(graph);
  const present = presentNodes(graph, view.callersOpen);
  const visible = visibleBoxes(tree, presentBoxes(tree, present), view.expanded);
  return {
    tree,
    visible,
    badges: new Set<string>(),
    links: links(graph, tree, present, view.expanded),
  };
}

describe("drill-down tree", () => {
  const graph = graphOf(
    [
      ["src/review/a.rs", "S", { kind: "class" }],
      ["src/review/a.rs", "run", { parent: "S" }],
      ["src/review/a.rs", "stop", { parent: "Unchanged" }],
      ["src/review/a.rs", "free"],
    ],
    [],
  );
  const { boxes, roots } = buildTree(graph);

  test("merges single-folder chains and nests members under their enclosing symbol", () => {
    expect(roots).toEqual(["d:src/review"]);
    expect(boxes.get("d:src/review")?.label).toBe("src/review");
    expect(boxes.get("d:src/review")?.children).toEqual(["f:src/review/a.rs"]);
    expect(boxes.get("f:src/review/a.rs")?.label).toBe("a.rs");
    // A changed class is its group's own symbol; an unchanged one is just a group.
    expect(boxes.get("f:src/review/a.rs")?.children).toEqual([
      "g:src/review/a.rs#S",
      "s:3",
      "g:src/review/a.rs#Unchanged",
    ]);
    expect(boxes.get("g:src/review/a.rs#S")?.own).toEqual([0]);
    expect(boxes.get("g:src/review/a.rs#S")?.children).toEqual(["s:1"]);
    expect(boxes.get("g:src/review/a.rs#Unchanged")?.own).toEqual([]);
  });
});

describe("collapsed boxes", () => {
  const graph = graphOf(
    [
      ["a.rs", "one"],
      ["a.rs", "two"],
      ["b.rs", "three"],
      ["b.rs", "four"],
    ],
    [
      [0, 2],
      [1, 3],
      [1, 0],
    ],
  );

  test("draw one link between two closed files, however many uses are under it", () => {
    const closed = scene(graph, { expanded: new Set(), callersOpen: new Set() });
    expect(closed.visible).toEqual(["f:a.rs", "f:b.rs"]);
    expect(closed.links).toEqual([{ from: "f:a.rs", to: "f:b.rs", caller: false }]);
  });

  test("leave out a link between a box and what's inside it", () => {
    const g = graphOf(
      [
        ["a.rs", "S", { kind: "class" }],
        ["a.rs", "run", { parent: "S" }],
      ],
      [[1, 0]],
    );
    const open = scene(g, { expanded: new Set(["f:a.rs", "g:a.rs#S"]), callersOpen: new Set() });
    expect(open.visible).toEqual(["f:a.rs", "g:a.rs#S", "s:1"]);
    expect(open.links).toEqual([]);
  });

  test("open files link their symbols", () => {
    const open = scene(graph, { expanded: new Set(["f:a.rs", "f:b.rs"]), callersOpen: new Set() });
    expect(open.links.map((l) => `${l.from}>${l.to}`).sort()).toEqual([
      "s:0>s:2",
      "s:1>s:0",
      "s:1>s:3",
    ]);
  });

  test("the first view opens folders until the graph would pass 20 boxes", () => {
    const many = graphOf(
      Array.from({ length: 30 }, (_, i): [string, string] => [`d${i % 2}/f${i}.rs`, `fn${i}`]),
      [],
    );
    expect([...firstView(many, buildTree(many)).expanded]).toEqual([]);
    const few = graphOf(
      [
        ["src/a.rs", "one"],
        ["lib/b.rs", "two"],
      ],
      [],
    );
    expect([...firstView(few, buildTree(few)).expanded].sort()).toEqual(["d:lib", "d:src"]);
  });
});

describe("callers", () => {
  const graph = graphOf(
    [
      ["a.rs", "helper"],
      ["b/c.rs", "uses_helper", { hop: 1 }],
      ["d.rs", "outer", { hop: 2 }],
    ],
    [
      [1, 0],
      [2, 1],
    ],
  );
  const tree = buildTree(graph);

  test("start hidden and come in one hop at a time, opening the boxes around them", () => {
    const start: View = { expanded: new Set(["f:a.rs"]), callersOpen: new Set() };
    expect(scene(graph, start).visible).toEqual(["f:a.rs", "s:0"]);

    const one = toggleCallers(graph, tree, start, 0);
    expect(scene(graph, one).visible).toEqual(["d:b", "f:b/c.rs", "s:1", "f:a.rs", "s:0"]);
    expect(scene(graph, one).links).toEqual([{ from: "s:1", to: "s:0", caller: true }]);

    const two = toggleCallers(graph, tree, one, 1);
    expect(scene(graph, two).visible).toContain("s:2");
    expect(scene(graph, toggleCallers(graph, tree, two, 0)).visible).toEqual(["f:a.rs", "s:0"]);
  });
});

test("lays out symbols inside their open file's box, users left of what they use", async () => {
  const graph = graphOf(
    [
      ["a.rs", "helper"],
      ["b.rs", "user"],
    ],
    [[1, 0]],
  );
  const view: View = { expanded: new Set(["f:a.rs"]), callersOpen: new Set() };
  const s = scene(graph, view);
  const placed = await layout({ ...s, open: new Set(["f:a.rs"]) });
  const a = placed.boxes.get("f:a.rs");
  const b = placed.boxes.get("f:b.rs");
  const helper = placed.boxes.get("s:0");
  expect(a && b && helper).toBeTruthy();
  if (!a || !b || !helper) return;
  expect(b.abs.x + b.abs.width).toBeLessThanOrEqual(a.abs.x);
  // Inside its box, in the box's coordinates.
  expect(helper.at.x + helper.at.width).toBeLessThanOrEqual(a.at.width);
  expect(helper.abs.x).toBe(a.abs.x + helper.at.x);
});
