import { describe, expect, it } from "vitest";
import type { FileDto } from "../bindings";
import { decorate } from "./decor";

const file = (path: string, status: FileDto["status"], conflict = false) =>
  ({ path, status, conflict }) as FileDto;

describe("decorate", () => {
  it("colors folders by their most urgent file: conflict, then added, then modified", () => {
    const { byFile, byDir } = decorate([
      file("src/a/x.ts", "modified"),
      file("src/a/y.ts", "added"),
      file("src/b/z.ts", "modified", true),
      file("docs/old.md", "removed"),
      file("README.md", "copied"),
    ]);
    expect(byDir.get("src/a")).toBe("added");
    expect(byDir.get("src")).toBe("conflict");
    expect(byDir.has("docs")).toBe(false);
    expect(byFile.get("README.md")).toBe("added");
    expect(byFile.get("src/a/x.ts")).toBe("modified");
  });
});
