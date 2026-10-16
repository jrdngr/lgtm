import { describe, expect, test } from "vitest";
import { archOf, comp, edge } from "./fixture.test-utils";
import { buildScene, containerEdges, focusOf } from "./scene";

const ids = (xs: { id: string }[]) => xs.map((x) => x.id).sort();
const keys = (xs: { key: string }[]) => xs.map((x) => x.key).sort();

describe("Changed and All", () => {
  const arch = archOf(
    [
      comp({ id: "review", status: "changed" }),
      comp({ id: "github", status: "upstream" }),
      comp({ id: "mcp", status: "impacted" }),
      comp({ id: "jj" }),
      comp({ id: "store" }),
    ],
    [edge("review", "github"), edge("mcp", "review"), edge("review", "jj"), edge("jj", "store")],
  );

  test("Changed keeps changed, upstream and impacted components and the edges among them", () => {
    const s = buildScene(arch, "changed");
    expect(ids(s.components)).toEqual(["github", "mcp", "review"]);
    expect(keys(s.edges)).toEqual(["mcp>review", "review>github"]);
  });

  test("All keeps every component and edge", () => {
    const s = buildScene(arch, "all");
    expect(ids(s.components)).toHaveLength(5);
    expect(s.edges).toHaveLength(4);
  });

  test("Changed still shows unchanged components that a Wrong or Move mark names", () => {
    const marked = archOf(
      [
        comp({ id: "review", status: "changed", misplaced: ["src/review/mod.rs#threads"] }),
        comp({ id: "jj" }),
        comp({ id: "pr" }),
        comp({ id: "store" }),
      ],
      [edge("review", "jj", { wrong: 1 })],
      [
        { id: 1, state: "broken", kind: "wrong", from: "review", to: "jj", reason: null },
        {
          id: 2,
          state: "open",
          kind: "move",
          path: "src/review/mod.rs",
          symbol: "threads",
          to: "pr",
        },
      ],
    );
    const s = buildScene(marked, "changed");
    expect(ids(s.components)).toEqual(["jj", "pr", "review"]);
    expect(s.edges.map((e) => [e.key, e.style])).toEqual([
      ["review>jj", "wrong"],
      ["move:2", "move"],
    ]);
  });

  test("containers with nothing shown are left out", () => {
    const two = archOf([comp({ id: "a", status: "changed" }), comp({ id: "b", container: "ui" })]);
    expect(buildScene(two, "changed").groups.map((g) => g.label)).toEqual(["src"]);
    expect(buildScene(two, "all").groups.map((g) => g.label)).toEqual(["src", "ui"]);
  });
});

describe("edge styles", () => {
  const arch = archOf(
    ["a", "b", "c", "d", "e", "f"].map((id) => comp({ id, status: "changed" })),
    [
      edge("a", "b", { status: "added" }),
      edge("a", "c", { status: "removed" }),
      edge("a", "d", { impacted: true }),
      edge("a", "e", { status: "added", wrong: 3 }),
      edge("a", "f", { status: "removed", wrong: 4 }),
    ],
  );
  test("a broken Wrong mark wins, and a removed edge no longer breaks one", () => {
    const style = Object.fromEntries(buildScene(arch, "all").edges.map((e) => [e.to, e.style]));
    expect(style).toEqual({ b: "add", c: "rem", d: "imp", e: "wrong", f: "rem" });
  });
});

describe("container links", () => {
  const arch = archOf(
    [
      comp({ id: "a", container: "core" }),
      comp({ id: "b", container: "core" }),
      comp({ id: "x", container: "ui" }),
      comp({ id: "y", container: "ui" }),
    ],
    [
      edge("x", "a", { status: "removed" }),
      edge("y", "b", { status: "removed" }),
      edge("a", "x", { status: "removed" }),
      edge("b", "y"),
      edge("a", "b", { status: "added" }),
    ],
  );
  test("one link per container pair, gone only when every edge under it is", () => {
    const links = containerEdges(arch, buildScene(arch, "all").edges);
    expect(links.map((l) => [l.key, l.style]).sort()).toEqual([
      ["c:core>c:ui", "ctx"],
      ["c:ui>c:core", "rem"],
    ]);
  });
});

describe("focus", () => {
  const arch = archOf(
    [comp({ id: "a" }), comp({ id: "b" }), comp({ id: "c" }), comp({ id: "d" })],
    [edge("a", "b"), edge("b", "c"), edge("c", "d")],
    [],
    {
      layers: [{ n: 1, title: "", summary: "", components: ["b", "c"], ranges: [] }],
    },
  );
  const edges = buildScene(arch, "all").edges;

  test("a component lights its edges and their other ends", () => {
    const f = focusOf(arch, edges, { kind: "component", id: "b" });
    expect([...(f?.nodes ?? [])].sort()).toEqual(["a", "b", "c"]);
    expect([...(f?.edges ?? [])].sort()).toEqual(["a>b", "b>c"]);
  });

  test("a layer lights its components and only the edges between them", () => {
    const f = focusOf(arch, edges, { kind: "layer", n: 1 });
    expect([...(f?.nodes ?? [])].sort()).toEqual(["b", "c"]);
    expect([...(f?.edges ?? [])]).toEqual(["b>c"]);
  });
});
