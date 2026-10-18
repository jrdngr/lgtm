import { describe, expect, it, vi } from "vitest";
import { linksIn, resolveLink } from "./links";

vi.mock("./open", () => ({ openFile() {} }));
vi.mock("../services/pathMenu", () => ({ openUrl() {} }));

describe("linksIn", () => {
  it("finds markdown links, autolinks and bare addresses with their spans", () => {
    const line = 'See [the spec](docs/spec.md "Spec"), <https://a.dev/x> and https://b.dev/y.';
    expect(linksIn(line)).toEqual([
      { from: 4, to: 35, href: "docs/spec.md" },
      { from: 37, to: 54, href: "https://a.dev/x" },
      { from: 59, to: 74, href: "https://b.dev/y" },
    ]);
  });

  it("ignores brackets that aren't links", () => {
    expect(linksIn("- [x] done, a[0] (b)")).toEqual([]);
  });
});

describe("resolveLink", () => {
  it("keeps web addresses and resolves paths against the file's folder", () => {
    expect(resolveLink("https://a.dev", "docs/a.md")).toEqual({ url: "https://a.dev" });
    expect(resolveLink("../src/my%20file.ts#L3", "docs/a.md")).toEqual({
      path: "src/my file.ts",
    });
    expect(resolveLink("./b.md", "docs/a.md")).toEqual({ path: "docs/b.md" });
    expect(resolveLink("/README.md", "docs/a.md")).toEqual({ path: "README.md" });
  });

  it("goes nowhere for anchors, other schemes, or paths without a file to start from", () => {
    expect(resolveLink("#usage", "docs/a.md")).toBeUndefined();
    expect(resolveLink("javascript:alert(1)", "docs/a.md")).toBeUndefined();
    expect(resolveLink("b.md", undefined)).toBeUndefined();
  });
});
