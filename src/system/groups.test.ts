import { describe, expect, it } from "vitest";
import type { SystemDescDto, SystemDto } from "../bindings";
import { groupsOf } from "./groups";

function map(ids: string[], groups?: SystemDescDto["groups"]): SystemDto {
  return {
    repo: "mono",
    containers: ids.map((id) => ({ id, name: id.split("/").pop() ?? id, files: 1 })),
    components: [],
    edges: [],
    files: [],
    tours: [],
    stale: [],
    description: groups
      ? {
          summary: "",
          people: [],
          externals: [],
          containers: [],
          tiers: [],
          components: [],
          groups,
          relations: [],
          commit: "c",
          createdAt: 0,
        }
      : null,
  };
}

const named = (m: SystemDto) => groupsOf(m)?.map((g) => [g.name, g.containers]);

describe("groupsOf", () => {
  it("takes the description's groups, with the containers it left out in Other", () => {
    const m = map(
      ["apps/web", "services/scribe", "services/bridge"],
      [
        { name: "Voice", description: "Calls", containers: ["services/bridge"] },
        { name: "Notes", description: "Notes", containers: ["services/scribe", "apps/web"] },
        { name: "Empty", description: "", containers: [] },
      ],
    );
    m.description?.containers.push({ id: "db", name: "DB", tech: "", description: "" });
    expect(named(m)).toEqual([
      ["Voice", ["services/bridge"]],
      ["Notes", ["apps/web", "services/scribe"]],
      ["Other", ["db"]],
    ]);
  });

  it("falls back to each container's parent folder, top-level ones under the repo", () => {
    expect(
      named(map(["apps/web", "services/a", "services/b", "libs/core/py", "tools", "x"])),
    ).toEqual([
      ["apps", ["apps/web"]],
      ["services", ["services/a", "services/b"]],
      ["core", ["libs/core/py"]],
      ["mono", ["tools", "x"]],
    ]);
  });

  it("has no groups for a few containers, or when everything lands in one", () => {
    expect(groupsOf(map(["apps/web", "services/a", "libs/b", "c", "d"]))).toBeUndefined();
    expect(groupsOf(map(["s/a", "s/b", "s/c", "s/d", "s/e", "s/f"]))).toBeUndefined();
  });
});
