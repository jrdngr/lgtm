import { describe, expect, it } from "vitest";
import type { SystemDto, TourStepDto } from "../bindings";
import { stopOf } from "./tours";

const comp = (id: string, container: string) => ({
  id,
  name: id.split("/").pop() ?? id,
  container,
  files: 1,
  entry: [],
});

const map: SystemDto = {
  repo: "app",
  containers: [
    { id: "web", name: "Web", files: 2 },
    { id: "core", name: "Core", files: 1 },
  ],
  components: [comp("web/ui", "web"), comp("web/review", "web"), comp("core/db", "core")],
  edges: [],
  files: [
    { path: "web/review/current.ts", component: "web/review" },
    { path: "core/db/mod.rs", component: "core/db" },
  ],
  tours: [],
  description: null,
  stale: [],
};

const colors = new Map([
  ["web", "#5b9bd5"],
  ["core", "#e8913f"],
]);

const step = (s: Partial<TourStepDto>): TourStepDto => ({ node: "system", text: "", ...s });

const at = (s: Partial<TourStepDto>) => {
  const stop = stopOf(map, colors, "k:web/review", step(s), 0);
  return stop && { level: stop.level, node: stop.node, edge: stop.edge, kind: stop.kind };
};

describe("stopOf", () => {
  it("shows a node at the level that holds it", () => {
    expect(at({ node: "system" })).toMatchObject({ level: { kind: "context" }, kind: "Context" });
    expect(at({ node: "web" })).toMatchObject({ level: { kind: "containers" }, node: "c:web" });
    expect(at({ node: "web/review" })).toMatchObject({
      level: { kind: "components", container: "web" },
      node: "k:web/review",
      kind: "Component",
    });
  });

  it("takes files and symbols to the Symbols level of the file's component", () => {
    expect(at({ node: "web", path: "web/review/current.ts" })).toMatchObject({
      level: { kind: "symbols", component: "web/review" },
      kind: "File",
    });
    const s = stopOf(
      map,
      colors,
      "k:web/review",
      step({ node: "web/review", path: "web/review/current.ts", symbol: "useReview", line: 4 }),
      0,
    );
    expect(s).toMatchObject({
      kind: "Symbol",
      name: "useReview",
      symbol: { path: "web/review/current.ts", name: "useReview", line: 4 },
    });
  });

  it("names an edge by which end is the tour's subject", () => {
    expect(at({ node: "web/ui", to: "web/review" })).toMatchObject({
      level: { kind: "components", container: "web" },
      edge: { from: "k:web/ui", to: "k:web/review" },
      kind: "Who uses it",
    });
    expect(at({ node: "web/review", to: "core/db" })).toMatchObject({
      level: { kind: "components", container: "web" },
      kind: "What it uses",
    });
    expect(at({ node: "web", to: "core" })).toMatchObject({ level: { kind: "containers" } });
  });

  it("shows the other end of an edge to or from the system", () => {
    expect(at({ node: "system", to: "web" })).toMatchObject({
      level: { kind: "containers" },
      node: "c:web",
      edge: undefined,
    });
  });

  it("skips a step whose node the map lost", () => {
    expect(at({ node: "gone" })).toBeUndefined();
  });
});
