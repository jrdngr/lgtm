import { describe, expect, it } from "vitest";
import type { SystemDescDto, SystemDto } from "../bindings";
import { layout } from "./layout";
import { computedTiers, type Overlay, type Scene, sceneOf } from "./scene";

const comp = (id: string, container: string, files = 1) => ({
  id,
  name: id.split("/").pop() ?? id,
  container,
  files,
  entry: [],
});

function map(description: SystemDescDto | null = null): SystemDto {
  return {
    repo: "app",
    containers: [
      { id: "src", name: "app", files: 5 },
      { id: "core/src", name: "core", files: 4 },
    ],
    components: [
      comp("src/ui", "src", 3),
      comp("src/api", "src", 2),
      comp("core/src/db", "core/src", 4),
    ],
    edges: [
      { from: "src/ui", to: "src/api", imports: 6, path: "src/ui/a.ts", line: 1 },
      { from: "src/api", to: "core/src/db", imports: 2, path: "src/api/b.ts", line: 1 },
      { from: "src/ui", to: "core/src/db", imports: 1, path: "src/ui/a.ts", line: 2 },
    ],
    files: [],
    description,
    stale: [],
  };
}

function described(): SystemDescDto {
  return {
    summary: "An app",
    people: [{ name: "User", description: "Uses it" }],
    externals: [{ name: "GitHub", kind: "API", description: "PRs" }],
    containers: [
      { id: "src", name: "Web", tech: "React", description: "Screens" },
      { id: "store", name: "Store", tech: "SQLite", description: "State" },
    ],
    tiers: ["Entry", "Platform"],
    components: [
      { id: "src/ui", description: "Screens", tier: "Entry" },
      { id: "src/api", description: "Calls", tier: null },
    ],
    relations: [
      { from: "User", to: "src", label: "Reviews" },
      { from: "src", to: "GitHub", label: "Reads PRs" },
      { from: "core/src", to: "GitHub", label: "Posts" },
      { from: "src", to: "store", label: "Saves" },
    ],
    tour: [],
    commit: "c1",
    createdAt: 0,
  };
}

const ids = (s: Scene) => ({
  left: s.left.map((n) => n.id),
  inside: s.inside.map((r) => [r.label, r.nodes.map((n) => n.id)]),
  right: s.right.map((n) => n.id),
});

const edges = (s: Scene) =>
  s.edges.map((e) => [e.from, e.to, e.imports, e.label, e.status].filter((x) => x !== undefined));

describe("sceneOf", () => {
  it("draws context relations to the system, merging what its containers say", () => {
    const s = sceneOf(map(described()), { kind: "context" });
    expect(ids(s)).toEqual({
      left: ["p:User"],
      inside: [[undefined, ["sys"]]],
      right: ["x:GitHub"],
    });
    expect(edges(s)).toEqual([
      ["p:User", "sys", 0, "Reviews"],
      ["sys", "x:GitHub", 0, "Reads PRs · Posts"],
    ]);
  });

  it("sums imports between containers and adds described ones without code", () => {
    const s = sceneOf(map(described()), { kind: "containers" });
    expect(ids(s).inside).toEqual([[undefined, ["c:src", "c:core/src", "c:store"]]]);
    expect(s.inside[0].nodes[0]).toMatchObject({ name: "Web", tech: "React", tiles: [3, 2] });
    expect(edges(s)).toEqual([
      ["c:src", "c:core/src", 3],
      ["p:User", "c:src", 0, "Reviews"],
      ["c:src", "x:GitHub", 0, "Reads PRs"],
      ["c:core/src", "x:GitHub", 0, "Posts"],
      ["c:src", "c:store", 0, "Saves"],
    ]);
  });

  it("rows a container's components by described tier, with containers they import as doors", () => {
    const s = sceneOf(map(described()), { kind: "components", container: "src" });
    expect(ids(s).inside).toEqual([
      ["Entry", ["k:src/ui"]],
      ["Other", ["k:src/api"]],
    ]);
    expect(ids(s).right).toEqual(["c:core/src"]);
    expect(edges(s)).toEqual([
      ["k:src/ui", "k:src/api", 6],
      ["k:src/api", "c:core/src", 2],
      ["k:src/ui", "c:core/src", 1],
    ]);
  });

  it("marks the change and adds what it removed", () => {
    const overlay: Overlay = {
      status: new Map([
        ["src/ui", "changed"],
        ["src/old", "removed"],
      ]),
      removed: [comp("src/old", "src")],
      edges: [
        { from: "src/ui", to: "src/api", status: "added" },
        { from: "src/old", to: "src/api", status: "removed" },
      ],
    };
    const s = sceneOf(map(), { kind: "components", container: "src" }, overlay);
    expect(ids(s).inside).toEqual([
      [undefined, ["k:src/ui"]],
      [undefined, ["k:src/api"]],
      ["Removed", ["k:src/old"]],
    ]);
    expect(s.inside[0].nodes[0].status).toBe("changed");
    expect(edges(s).filter(([, to]) => to !== "c:core/src")).toEqual([
      ["k:src/ui", "k:src/api", 6, "added"],
      ["k:src/old", "k:src/api", 0, "removed"],
    ]);
    const c = sceneOf(map(), { kind: "containers" }, overlay);
    expect(c.inside[0].nodes.map((n) => n.status)).toEqual(["changed", undefined]);
  });
});

describe("computedTiers", () => {
  it("puts mostly importing components on top and mostly imported ones at the bottom", () => {
    const tiers = computedTiers(
      ["app", "feature", "util", "alone"],
      [
        { from: "app", to: "feature", imports: 5 },
        { from: "feature", to: "util", imports: 4 },
        { from: "util", to: "feature", imports: 1 },
        { from: "app", to: "util", imports: 2 },
        { from: "outside", to: "alone", imports: 9 },
      ],
    );
    expect(Object.fromEntries(tiers)).toEqual({ app: 0, feature: 1, util: 2, alone: 1 });
  });
});

describe("layout", () => {
  it("keeps the boundary's rows inside it, with people left of it and externals right", () => {
    const l = layout(sceneOf(map(described()), { kind: "containers" }));
    const b = l.boundary;
    if (!b) throw new Error("no boundary");
    const inside = l.nodes.filter((n) => n.kind === "container");
    for (const n of inside) {
      expect(n.x).toBeGreaterThan(b.x);
      expect(n.x + n.w).toBeLessThan(b.x + b.w);
      expect(n.y).toBeGreaterThan(b.y);
      expect(n.y + n.h).toBeLessThan(b.y + b.h);
    }
    const [p] = l.nodes.filter((n) => n.kind === "person");
    const [x] = l.nodes.filter((n) => n.kind === "external");
    expect(p.x + p.w).toBeLessThan(b.x);
    expect(x.x).toBeGreaterThan(b.x + b.w);
    expect(l.width).toBeGreaterThan(x.x + x.w);
  });
});
