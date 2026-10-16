import { describe, expect, test } from "vitest";
import type { GraphDto, GraphNode } from "../bindings";
import { archOf, comp } from "./fixture.test-utils";
import { scopedTree, scopeGraph } from "./scope";

const node = (id: number, path: string, name: string): GraphNode => ({
  id,
  name,
  kind: "function",
  status: "modified",
  path,
  file: 0,
  line: id + 1,
  rows: [0],
  parent: null,
  hop: 0,
});

const files = (...paths: string[]) => paths.map((path) => ({ path, file: 0 }));

describe("drilling into a component", () => {
  const arch = archOf(
    [
      comp({ id: "review", files: files("src/review/load.rs", "src/review/build.rs") }),
      comp({ id: "github", files: files("src/github/api.rs") }),
      comp({ id: "jj", files: files("src/jj/run.rs") }),
      comp({ id: "pr", files: files("src/pr/mod.rs") }),
    ],
    [],
    [
      {
        id: 1,
        state: "open",
        kind: "move",
        path: "src/review/load.rs",
        symbol: "threads",
        to: "pr",
      },
    ],
  );
  const graph: GraphDto = {
    nodes: [
      node(0, "src/review/load.rs", "load"),
      node(1, "src/review/load.rs", "threads"),
      node(2, "src/github/api.rs", "client"),
      node(3, "src/github/api.rs", "fetch"),
      node(4, "src/jj/run.rs", "run"),
      node(5, "src/review/build.rs", "build"),
    ],
    edges: [
      { from: 1, to: 2 },
      { from: 1, to: 3 },
      { from: 0, to: 1 },
      { from: 4, to: 3 },
      { from: 5, to: 4 },
    ],
  };
  const scoped = scopeGraph(graph, arch, "review");
  const name = (id: number) => scoped.graph.nodes[id].name;

  test("keeps the component's symbols and stands in one stub for each other component", () => {
    expect(scoped.graph.nodes.map((n) => n.name)).toEqual([
      "load",
      "threads",
      "build",
      "github",
      "jj",
      "pr",
    ]);
    expect([...scoped.stubs.values()]).toEqual(["github", "jj", "pr"]);
  });

  test("uses of another component's symbols become one use of its stub", () => {
    const edges = scoped.graph.edges.map((e) => `${name(e.from)}>${name(e.to)}`).sort();
    expect(edges).toEqual(["build>jj", "load>threads", "threads>github"]);
  });

  test("stubs sit at the top level, not in a file", () => {
    const tree = scopedTree(scoped);
    for (const id of scoped.stubs.keys()) {
      const box = tree.boxes.get(tree.boxOf[id]);
      expect(box?.parent).toBeUndefined();
      expect(tree.roots).toContain(box?.id);
    }
    expect(
      [...tree.boxes.values()]
        .filter((b) => b.kind === "file")
        .map((b) => b.label)
        .sort(),
    ).toEqual(["build.rs", "load.rs"]);
  });
});
