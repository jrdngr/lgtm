// @vitest-environment jsdom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";
import type { SystemDto } from "../bindings";
import { TraceView } from "./TraceView";
import { drawn, shownAs, tracePath, tracePaths } from "./trace";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

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
  grouped: true,
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

  it("never takes a relation out and straight back", () => {
    const apart = { ...map, edges: [] };
    expect(tracePath(apart, "k:web/ui", "k:web/api")).toBeUndefined();
    expect(tracePaths(apart, "k:web/ui", "k:web/api")).toEqual([]);
  });

  it("finds no hops between a container and a component in it", () => {
    expect(tracePath(map, "c:web", "k:web/ui")).toEqual([]);
    expect(tracePath(map, "k:web/ui", "c:web")).toEqual([]);
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

  it("puts the shortest path first however many longer ones come before it", () => {
    const bs = Array.from({ length: 60 }, (_, i) => `b${i}`);
    const fan: SystemDto = {
      ...grid,
      components: ["a", "z", ...bs].map((k) => comp(k, "app")),
      edges: [
        ...bs.flatMap((b) => [edge("a", b, "a.ts"), edge(b, "z", `${b}.ts`)]),
        edge("a", "z", "a.ts"),
      ],
    };
    expect(names(tracePaths(fan, "k:a", "k:z"))).toEqual([
      "az",
      "ab0 b0z",
      "ab1 b1z",
      "ab2 b2z",
      "ab3 b3z",
    ]);
  });
});

describe("TraceView", () => {
  const render = (from: string, to: string) => {
    const el = document.createElement("div");
    const all = tracePaths(map, from, to);
    const props = {
      map,
      colors: new Map<string, string>(),
      from,
      to,
      hops: all[0] ?? [],
      all,
      show: "shortest" as const,
      onMode() {},
      onShow() {},
      onFile() {},
    };
    act(() => createRoot(el).render(createElement(TraceView, props)));
    return el;
  };

  it("draws a container and a component in it joined, and says when nothing joins two nodes", () => {
    const held = render("c:web", "k:web/ui");
    expect(held.querySelector(".sys-via")).toBeNull();
    expect(held.querySelectorAll(".sys-prow .sys-pn")).toHaveLength(2);
    expect(held.querySelector(".sys-prow .sys-pin")).not.toBeNull();
    const apart = render("p:User", "x:Nowhere");
    expect(apart.querySelector(".sys-via")?.textContent).toBe("No path");
  });
});
