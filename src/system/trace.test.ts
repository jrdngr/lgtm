import { describe, expect, it } from "vitest";
import type { SystemDto } from "../bindings";
import { drawn, shownAs, tracePath } from "./trace";

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
