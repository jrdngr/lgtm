import { describe, expect, it } from "vitest";
import type { SystemDto } from "../bindings";
import { drawn, shownAs, tracePath, tracePaths } from "./trace";

const comp = (id: string, container: string) => ({
  id,
  name: id.split("/").pop() ?? id,
  container,
  files: 1,
  entry: [],
});

const edge = (from: string, to: string, path: string) => ({
  from,
  to,
  imports: 1,
  path,
  line: 3,
});

const map: SystemDto = {
  repo: "app",
  containers: [
    { id: "web", name: "Web", files: 2 },
    { id: "core", name: "Core", files: 2 },
  ],
  components: [comp("web/ui", "web"), comp("web/api", "web"), comp("core/db", "core")],
  edges: [edge("web/ui", "web/api", "web/ui/a.ts"), edge("web/api", "core/db", "web/api/b.ts")],
  files: [],
  tours: [],
  description: {
    summary: "",
    people: [{ name: "User", description: "" }],
    externals: [{ name: "Postgres", kind: "Database", description: "" }],
    containers: [],
    tiers: [],
    components: [],
    relations: [
      { from: "User", to: "web", label: "Uses" },
      { from: "core", to: "Postgres", label: "Queries" },
    ],
    tour: [],
    commit: "c",
    createdAt: 0,
  },
  stale: [],
};

describe("tracePath", () => {
  it("follows imports and relations forward, passing freely between a container and its components", () => {
    expect(tracePath(map, "p:User", "x:Postgres")).toEqual([
      { from: "p:User", to: "c:web", label: "Uses" },
      { from: "k:web/api", to: "k:core/db", path: "web/api/b.ts", line: 3 },
      { from: "c:core", to: "x:Postgres", label: "Queries" },
    ]);
  });

  it("goes against the arrows only when nothing leads forward", () => {
    expect(tracePath(map, "k:core/db", "k:web/ui")).toEqual([
      { from: "k:web/api", to: "k:core/db", path: "web/api/b.ts", line: 3 },
      { from: "k:web/ui", to: "k:web/api", path: "web/ui/a.ts", line: 3 },
    ]);
  });
});

describe("drawn", () => {
  it("rolls a path up to the level's nodes and numbers its edges in order", () => {
    const hops = tracePath(map, "k:web/ui", "x:Postgres") ?? [];
    const d = drawn(map, { kind: "containers" }, hops);
    expect([...d.nodes]).toEqual(["c:web", "c:core", "x:Postgres"]);
    expect([...d.edges]).toEqual([
      ["c:web>c:core", 1],
      ["c:core>x:Postgres", 2],
    ]);
  });

  it("shows another container's components as that container inside a components view", () => {
    const level = { kind: "components", container: "web" } as const;
    expect(shownAs(map, level, "k:web/ui")).toBe("k:web/ui");
    expect(shownAs(map, level, "k:core/db")).toBe("c:core");
    expect(shownAs(map, level, "c:web")).toBeUndefined();
    expect(shownAs(map, level, "p:User")).toBeUndefined();
  });
});

describe("tracePaths", () => {
  // a reaches d directly, through b, through c, through c and e, and through f, g and h.
  const pairs = ["ab", "bd", "ac", "cd", "ad", "ce", "ed", "af", "fg", "gh", "hd"];
  const grid: SystemDto = {
    ...map,
    containers: [{ id: "app", name: "App", files: 8 }],
    components: [..."abcdefgh"].map((k) => comp(k, "app")),
    edges: pairs.map(([a, b]) => edge(a, b, `${a}.ts`)),
    description: null,
  };
  const names = (paths: { from: string; to: string }[][]) =>
    paths.map((p) => p.map((h) => `${h.from.slice(2)}${h.to.slice(2)}`).join(" "));

  it("lists paths up to two hops longer than the shortest, fewest hops first, up to the cap", () => {
    expect(names(tracePaths(grid, "k:a", "k:d"))).toEqual(["ad", "ab bd", "ac cd", "ac ce ed"]);
    expect(names(tracePaths(grid, "k:a", "k:d", 2))).toEqual(["ad", "ab bd"]);
  });
});
