import type { DockviewApi } from "dockview-react";
import { describe, expect, it } from "vitest";
import { registerDock } from "../core/docks";
import { type CurrentReview, publishReview } from "./current";
import { inReviewPane, markIdle } from "./idle";

function fakeDock(workspace: string) {
  const panels: {
    view: { contentComponent: string };
    shown: number;
    api: object;
    group: object;
  }[] = [];
  const api = {
    panels,
    groups: [],
    addPanel(o: { component: string }) {
      const p = { view: { contentComponent: o.component }, shown: 0, api: {}, group: {} };
      p.api = { setActive: () => p.shown++ };
      panels.push(p);
    },
  };
  registerDock(workspace, api as unknown as DockviewApi);
  return panels;
}

const review = (id: number) => ({ id }) as CurrentReview;

describe("inReviewPane", () => {
  it("opens a review pane and acts on it once it loads, not on the idle review", () => {
    const panels = fakeDock("a");
    const got: number[] = [];
    inReviewPane("a", (r) => got.push(r.id));
    expect(panels).toHaveLength(1);
    publishReview("a", markIdle(review(1)));
    publishReview("b", review(2));
    expect(got).toEqual([]);
    publishReview("a", review(3));
    publishReview("a", review(4));
    expect(got).toEqual([3]);
  });

  it("shows a loading pane instead of opening another, and only the latest action runs", () => {
    const panels = fakeDock("c");
    const got: string[] = [];
    inReviewPane("c", () => got.push("first"));
    inReviewPane("c", () => got.push("second"));
    expect(panels).toHaveLength(1);
    expect(panels[0].shown).toBe(1);
    publishReview("c", review(5));
    expect(got).toEqual(["second"]);
  });
});
