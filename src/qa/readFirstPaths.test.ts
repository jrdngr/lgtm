import { describe, expect, it } from "vitest";
import { addPaths, repoRelative, splitPath } from "./readFirstPaths";

describe("Read first paths", () => {
  it("stores paths inside the repo relative to it, and others whole", () => {
    expect(repoRelative("/r/app/src/a.rs", "/r/app")).toBe("src/a.rs");
    expect(repoRelative("/r/app/docs/", "/r/app/")).toBe("docs/");
    expect(repoRelative("/r/application/a.rs", "/r/app")).toBe("/r/application/a.rs");
    expect(repoRelative("/x/a.rs", undefined)).toBe("/x/a.rs");
  });

  it("shows the name first and the folder after it", () => {
    expect(splitPath("src-tauri/src/auth/refresh.rs")).toEqual({
      name: "refresh.rs",
      dir: "src-tauri/src/auth",
      folder: false,
    });
    expect(splitPath("docs/")).toEqual({ name: "docs", dir: "", folder: true });
    expect(splitPath("/notes.md")).toEqual({ name: "notes.md", dir: "/", folder: false });
  });

  it("adds new paths once, in order", () => {
    expect(addPaths(["a"], ["b", "a", "c", "b"])).toEqual(["a", "b", "c"]);
  });
});
