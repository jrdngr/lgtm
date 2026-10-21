import { describe, expect, it } from "vitest";
import { levelOfKey, viewKey } from "./view";

describe("levelOfKey", () => {
  it("reads back every level's key, ids with slashes included", () => {
    for (const level of [
      { kind: "context" },
      { kind: "containers" },
      { kind: "components", container: "apps/web" },
      { kind: "interfaces", component: "apps/web/src/api" },
    ] as const)
      expect(levelOfKey(viewKey(level))).toEqual(level);
  });
});
