// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import type { ProjectDto } from "../bindings";

vi.mock("../ipc", () => ({ commands: {} }));
const { parseUi, pickActive } = await import("./store");

const repo = (label: string, asleep = false) => ({
  label,
  root: `/r/${label}`,
  name: label,
  asleep,
});
const project = (...repos: ReturnType<typeof repo>[]): ProjectDto => ({
  id: "p",
  name: "p",
  repos,
  ui: "{}",
  active: null,
});

describe("parseUi", () => {
  it("keeps valid fields and drops unknown colors and non-strings", () => {
    const ui = parseUi(
      JSON.stringify({
        pinned: ["a", 3],
        colors: { a: "blue", b: "chartreuse" },
        collapsed: ["b"],
        active: "a",
      }),
    );
    expect(ui).toEqual({ pinned: ["a"], colors: { a: "blue" }, collapsed: ["b"], active: "a" });
  });

  it("falls back to empty state for unreadable text", () => {
    expect(parseUi("not json")).toEqual({ pinned: [], colors: {}, collapsed: [] });
    expect(parseUi("null")).toEqual({ pinned: [], colors: {}, collapsed: [] });
  });
});

describe("pickActive", () => {
  it("shows the wanted repo only when it's awake, else the first awake one", () => {
    const p = project(repo("a", true), repo("b"), repo("c"));
    expect(pickActive(p, "c")).toBe("c");
    expect(pickActive(p, "a")).toBe("b");
    expect(pickActive(p, null)).toBe("b");
  });

  it("is empty when every repo sleeps", () => {
    expect(pickActive(project(repo("a", true)), "a")).toBe("");
  });
});
