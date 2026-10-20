import { describe, expect, it } from "vitest";
import { NAV, navPush, navStep, type Place } from "./nav";

const context: Place = { level: { kind: "context" }, node: "sys" };
const containers: Place = { level: { kind: "containers" }, node: "c:src" };
const components: Place = { level: { kind: "components", container: "src" } };

describe("nav", () => {
  it("goes back to what was selected, and forward to where it left", () => {
    let nav = navPush(NAV, context, containers);
    nav = navPush(nav, { ...containers, node: "c:core" }, components);
    nav = navStep(nav, { ...components, node: "k:src/ui" }, -1);
    expect(nav.stack[nav.at]).toEqual({ ...containers, node: "c:core" });
    nav = navStep(nav, containers, 1);
    expect(nav.stack[nav.at]).toEqual({ ...components, node: "k:src/ui" });
  });

  it("drops the places forward of a new one", () => {
    let nav = navPush(NAV, context, containers);
    nav = navStep(nav, containers, -1);
    nav = navPush(nav, context, components);
    expect(nav.stack).toEqual([context, components]);
    expect(navStep(nav, components, 1)).toBe(nav);
  });

  it("stays put with nowhere to go", () => {
    expect(navStep(NAV, context, -1)).toBe(NAV);
    const nav = navPush(NAV, context, containers);
    expect(navStep(navStep(nav, containers, -1), context, -1).at).toBe(0);
  });
});
