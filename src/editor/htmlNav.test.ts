import { describe, expect, it } from "vitest";
import { appKey, fitted, pagePath, pageUrl, stepZoom, visit } from "./htmlNav";

const base = "lgtmhtml://localhost/";

describe("page URLs", () => {
  it("round-trip paths with spaces, unicode and #fragments", () => {
    const url = pageUrl(base, "r1", "docs/a b/ünï.html");
    expect(url).toBe("lgtmhtml://localhost/r1/docs/a%20b/%C3%BCn%C3%AF.html");
    expect(pagePath(base, "r1", `${url}?x=1#part`)).toBe("docs/a b/ünï.html#part");
  });

  it("encode characters a path can hold but a URL treats specially", () => {
    expect(pageUrl(base, "r1", "a#b?.html")).toBe("lgtmhtml://localhost/r1/a%23b%3F.html");
  });

  it("have no path under another root or off the scheme", () => {
    expect(pagePath(base, "r1", "lgtmhtml://localhost/r12/a.html")).toBeUndefined();
    expect(pagePath(base, "r1", "https://example.com/r1/a.html")).toBeUndefined();
  });
});

describe("visit", () => {
  it("drops the pages ahead when a new page shows after going back", () => {
    const trail = { urls: ["a", "b", "c"], at: 1 };
    expect(visit(trail, "d")).toEqual({ urls: ["a", "b", "d"], at: 2 });
  });

  it("keeps the trail when the page showing loads again", () => {
    const trail = { urls: ["a", "b"], at: 0 };
    expect(visit(trail, "a")).toBe(trail);
  });
});

describe("stepZoom", () => {
  it("moves to the neighboring step, from a fitted zoom between steps too", () => {
    expect(stepZoom(1, 1)).toBe(1.1);
    expect(stepZoom(1, -1)).toBe(0.9);
    expect(stepZoom(0.83, 1)).toBe(0.9);
    expect(stepZoom(0.83, -1)).toBe(0.8);
  });

  it("stays put past the ends", () => {
    expect(stepZoom(8, 1)).toBe(8);
    expect(stepZoom(0.2, -1)).toBe(0.2);
    expect(fitted(0.01)).toBe(0.1);
  });
});

describe("appKey", () => {
  const key = (
    k: string,
    mods: Partial<Record<"metaKey" | "ctrlKey" | "shiftKey" | "altKey", boolean>>,
  ) => ({
    key: k,
    code: "KeyX",
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    ...mods,
  });

  it("passes Escape and modifier shortcuts, with only the key fields", () => {
    expect(appKey(key("Escape", {}))).toEqual(key("Escape", {}));
    expect(appKey({ ...key("w", { metaKey: true }), bubbles: false })).toEqual(
      key("w", { metaKey: true }),
    );
    expect(appKey(key("c", { metaKey: true, shiftKey: true }))).toBeDefined();
  });

  it("drops plain keys, editing shortcuts and malformed messages", () => {
    expect(appKey(key("w", {}))).toBeUndefined();
    expect(appKey(key("Enter", { shiftKey: true }))).toBeUndefined();
    expect(appKey(key("C", { ctrlKey: true }))).toBeUndefined();
    expect(appKey({ key: "w", metaKey: true })).toBeUndefined();
    expect(appKey({ ...key("w", {}), metaKey: "yes" })).toBeUndefined();
  });
});
