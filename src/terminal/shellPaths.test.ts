import { describe, expect, it } from "vitest";
import { shellPaths } from "./shellPaths";

describe("shellPaths", () => {
  it("leaves plain paths bare and quotes ones with spaces, quotes and shell characters", () => {
    expect(shellPaths(["/a/b.txt", "/a/my file", "/a/it's", "/a/$(x)"])).toBe(
      `/a/b.txt '/a/my file' '/a/it'\\''s' '/a/$(x)' `,
    );
  });

  it("drops newlines so a dropped path can't run a command", () => {
    expect(shellPaths(["/a/x\nrm -rf ~"])).toBe("'/a/xrm -rf ~' ");
  });

  it("is empty for no paths", () => {
    expect(shellPaths([])).toBe("");
  });
});
