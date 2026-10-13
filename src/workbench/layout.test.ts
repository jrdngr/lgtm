import { describe, expect, it } from "vitest";
import { parseLayout } from "./layout";

describe("parseLayout", () => {
  it("reads a layout from before the bottom panel as the main dock's", () => {
    const old = { grid: { root: {} }, panels: {} };
    expect(parseLayout(JSON.stringify(old))).toEqual({ main: old });
  });

  it("keeps both docks of a current layout", () => {
    const both = { main: { grid: 1 }, bottom: { grid: 2 } };
    expect(parseLayout(JSON.stringify(both))).toEqual(both);
  });

  it("drops an unreadable layout", () => {
    expect(parseLayout("{oops")).toEqual({});
    expect(parseLayout("null")).toEqual({});
  });
});
