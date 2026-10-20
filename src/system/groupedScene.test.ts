import { describe, expect, it } from "vitest";
import type { SystemDto } from "../bindings";
import { layout } from "./layout";
import type { Lens } from "./lens";
import { type Scene, sceneOf } from "./scene";

const CONTAINERS = [
  "apps/web",
  "apps/admin",
  "services/bridge",
  "services/sms",
  "libs/core",
  "libs/db",
];

function map(): SystemDto {
  const comp = (c: string) => `${c}/main`;
  const edge = (a: string, b: string, imports = 1) => ({
    from: comp(a),
    to: comp(b),
    imports,
    path: `${a}/x.ts`,
    line: 1,
  });
  return {
    repo: "mono",
    containers: CONTAINERS.map((id) => ({ id, name: id.split("/")[1], files: 2 })),
    components: CONTAINERS.map((c) => ({
      id: comp(c),
      name: "main",
      container: c,
      files: 2,
      entry: [],
    })),
    edges: [
      edge("apps/web", "libs/core", 2),
      edge("apps/admin", "libs/core"),
      edge("services/bridge", "libs/core"),
      edge("services/sms", "services/bridge"),
      edge("libs/core", "libs/db"),
    ],
    files: [],
    tours: [],
    stale: [],
    description: null,
  };
}

const lens = (status: [string, "changed" | "impacted"][]): Lens => ({
  status: new Map(status),
  removed: [],
  edges: [],
  delta: new Map(),
  files: new Map(),
  look: new Map(),
  edgeLook: new Map(),
  open: 0,
});

const edges = (s: Scene, detail = false) =>
  s.edges.filter((e) => !!e.detail === detail).map((e) => `${e.from}>${e.to}:${e.imports}`);

describe("grouped scenes", () => {
  it("shows a large system's context as groups wired by their containers' imports", () => {
    const s = sceneOf(map(), { kind: "context" });
    expect(s.inside[0].nodes.map((n) => [n.id, n.blocks?.length])).toEqual([
      ["g:apps", 2],
      ["g:services", 2],
      ["g:libs", 2],
    ]);
    expect(s.pack).toBe(2);
    expect(edges(s)).toEqual(["g:apps>g:libs:3", "g:services>g:libs:1"]);
  });

  it("frames every group's containers, drawing groups' edges at rest and containers' when lit", () => {
    const s = sceneOf(map(), { kind: "containers" });
    expect(s.frames?.map((f) => [f.node.id, f.nodes.map((n) => [n.id, n.small])])).toEqual([
      [
        "g:apps",
        [
          ["c:apps/web", true],
          ["c:apps/admin", true],
        ],
      ],
      [
        "g:services",
        [
          ["c:services/bridge", true],
          ["c:services/sms", true],
        ],
      ],
      [
        "g:libs",
        [
          ["c:libs/core", true],
          ["c:libs/db", true],
        ],
      ],
    ]);
    expect(edges(s)).toEqual(["g:apps>g:libs:3", "g:services>g:libs:1"]);
    expect(edges(s, true)).toContain("c:apps/web>c:libs/core:2");
    expect(edges(s, true)).toContain("c:services/sms>c:services/bridge:1");
  });

  it("opens one group with the groups it talks to as doors", () => {
    const s = sceneOf(map(), { kind: "containers", group: "g:services" });
    expect(s.boundary).toBe("services");
    expect(s.inside.flatMap((r) => r.nodes.map((n) => n.id))).toEqual([
      "c:services/bridge",
      "c:services/sms",
    ]);
    expect(s.right.map((n) => [n.id, n.door])).toEqual([["g:libs", true]]);
    expect(edges(s)).toEqual(["c:services/bridge>g:libs:1", "c:services/sms>c:services/bridge:1"]);
  });

  it("keeps only the frames and cards the review affects", () => {
    const l = lens([
      ["apps/web/main", "changed"],
      ["libs/core/main", "impacted"],
    ]);
    const s = sceneOf(map(), { kind: "containers" }, l);
    expect(s.frames?.map((f) => [f.node.id, f.node.status, f.nodes.map((n) => n.id)])).toEqual([
      ["g:apps", "changed", ["c:apps/web"]],
      ["g:libs", "impacted", ["c:libs/core"]],
    ]);
    expect(edges(s)).toEqual(["g:apps>g:libs:3"]);
  });

  it("lays out frames under the cards they hold", () => {
    const l = layout(sceneOf(map(), { kind: "containers" }));
    const frame = l.nodes.find((n) => n.id === "g:apps");
    const card = l.nodes.find((n) => n.id === "c:apps/admin");
    if (!frame || !card) throw new Error("missing");
    expect(l.nodes.indexOf(frame)).toBeLessThan(l.nodes.indexOf(card));
    expect(card.x).toBeGreaterThan(frame.x);
    expect(card.x + card.w).toBeLessThanOrEqual(frame.x + frame.w);
    expect(card.y + card.h).toBeLessThanOrEqual(frame.y + frame.h);
  });
});
