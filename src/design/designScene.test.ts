import { describe, expect, it } from "vitest";
import type { DesignMapDto, SystemDto } from "../bindings";
import { designScene } from "./designScene";

const code: SystemDto = {
  repo: "shop",
  grouped: true,
  containers: [
    { id: "api", name: "api", files: 4 },
    { id: "web", name: "web", files: 3 },
  ],
  components: [
    { id: "api/orders", name: "orders", container: "api", files: 4, entry: [] },
    { id: "web/ui", name: "ui", container: "web", files: 3, entry: [] },
  ],
  edges: [{ from: "web/ui", to: "api/orders", imports: 3, path: "web/ui/a.ts", line: 1 }],
  files: [],
  tours: [],
  description: null,
  stale: [],
};

type Container = DesignMapDto["containers"][0];
const container = (id: string, change: Container["change"], built: Container["built"]) => ({
  id,
  name: id,
  tech: "",
  description: "",
  change,
  code: change === "new" ? null : id,
  replaces: [],
  spec: [],
  built,
});

function design(built: Container["built"] = null): DesignMapDto {
  return {
    summary: "Notifications",
    people: [],
    externals: [],
    containers: [container("api", "changed", built), container("notify", "new", built)],
    components: [
      {
        id: "notify/push",
        container: "notify",
        name: "push",
        description: "",
        change: "new",
        code: null,
        replaces: [],
        spec: [],
        interfaces: [
          { name: "send", signature: "send(user)", description: "", spec: [], built: null },
        ],
        built: null,
      },
    ],
    relations: [{ from: "api", to: "notify", label: "Publishes", change: "new", built: null }],
    differences: [],
    specCommit: "c1",
    codeCommit: "c0",
    filesHash: null,
    createdAt: 0,
  };
}

const marks = (s: ReturnType<typeof designScene>) =>
  s.inside.flatMap((r) => r.nodes.map((n) => [n.id, n.mark ?? null]));

describe("designScene", () => {
  it("lays the design over the code: changes marked, code-only containers kept, imports joined", () => {
    const s = designScene(design(), "Notifications", code, { kind: "containers" }, true);
    expect(marks(s)).toEqual([
      ["c:api", "changed"],
      ["c:notify", "new"],
      ["c:web", null],
    ]);
    expect(
      s.edges.map((e) => [e.from, e.to, e.imports, e.label ?? null, e.status ?? null]),
    ).toEqual([
      ["c:api", "c:notify", 0, "Publishes", "added"],
      ["c:web", "c:api", 3, null, null],
    ]);
  });

  it("shows the design alone without marks or code", () => {
    const s = designScene(design(), "Notifications", code, { kind: "containers" }, false);
    expect(marks(s)).toEqual([
      ["c:api", null],
      ["c:notify", null],
    ]);
    expect(s.edges.map((e) => [e.from, e.to])).toEqual([["c:api", "c:notify"]]);
  });

  it("marks by what's built once code has been compared", () => {
    const m = design("planned");
    m.containers[0] = { ...m.containers[0], built: "built" };
    const s = designScene(m, "Notifications", code, { kind: "containers" }, true);
    expect(marks(s).slice(0, 2)).toEqual([
      ["c:api", "built"],
      ["c:notify", "planned"],
    ]);
  });

  it("lists a component's interfaces on its card", () => {
    const s = designScene(
      design(),
      "N",
      undefined,
      { kind: "components", container: "notify" },
      true,
    );
    expect(s.inside[0]?.nodes.map((n) => n.lines)).toEqual([["send(user)"]]);
  });
});
