// @vitest-environment jsdom
import type { DockviewApi, DockviewGroupPanel } from "dockview-react";
import { describe, expect, it } from "vitest";
import { fold, foldAxis, foldOf, STRIP, savedFolds, unfold } from "./fold";

/** A group with the dockview calls folding makes, recording its constraints and size. */
function group(id: string, width = 400, height = 300) {
  const g = {
    id,
    constraints: {} as Record<string, number>,
    size: { width, height },
    element: document.createElement("div"),
    api: {
      width,
      height,
      setConstraints(c: Record<string, number>) {
        Object.assign(g.constraints, c);
      },
      setSize(s: { width?: number; height?: number }) {
        Object.assign(g.size, s);
      },
    },
  };
  return g;
}

/** Groups in a row, left to right. */
function row(...groups: ReturnType<typeof group>[]) {
  const api = {
    adjacentGroupInDirection(g: ReturnType<typeof group>, dir: string) {
      const i = groups.indexOf(g);
      if (dir === "left") return groups[i - 1];
      if (dir === "right") return groups[i + 1];
      return undefined;
    },
  };
  return api as unknown as DockviewApi;
}

const as = (g: ReturnType<typeof group>) => g as unknown as DockviewGroupPanel;

describe("fold", () => {
  it("pins a group to a strip and opens it back to its old width", () => {
    const [a, b] = [group("a", 420), group("b")];
    const api = row(a, b);
    fold(api, as(a));
    expect(a.constraints).toMatchObject({ minimumWidth: STRIP, maximumWidth: STRIP });
    expect(a.size.width).toBe(STRIP);
    expect(a.element.classList.contains("group-folded")).toBe(true);
    expect(savedFolds(api)).toEqual({ a: { axis: "x", size: 420 } });

    unfold(api, as(a));
    expect(a.constraints.maximumWidth).toBe(Number.POSITIVE_INFINITY);
    expect(a.size.width).toBe(420);
    expect(a.element.classList.contains("group-folded")).toBe(false);
    expect(foldOf(api, as(a))).toBeUndefined();
  });

  it("needs an open neighbor to give its space to", () => {
    const [a, b] = [group("a"), group("b")];
    const api = row(a, b);
    expect(foldAxis(row(a), as(a))).toBeUndefined();
    fold(api, as(a));
    expect(foldAxis(api, as(b))).toBeUndefined();
  });
});
