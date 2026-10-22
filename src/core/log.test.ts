import { describe, expect, it } from "vitest";
import { logText } from "./log";

describe("logText", () => {
  it("keeps an error's stack and writes other values as JSON", () => {
    const e = new Error("boom");
    expect(logText(e)).toContain("boom");
    expect(logText(e)).toBe(e.stack);
    expect(logText("plain")).toBe("plain");
    expect(logText({ a: [1] })).toBe('{"a":[1]}');
    expect(logText(undefined)).toBe("undefined");
  });

  it("falls back to String for values JSON can't write", () => {
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(logText(loop)).toBe("[object Object]");
  });
});
