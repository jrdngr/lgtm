import type { DockviewApi } from "dockview-react";
import { describe, expect, it } from "vitest";
import { registerDock } from "../workbench/docks";
import { type CurrentReview, publishReview } from "./current";
import { inReviewPane, markIdle } from "./idle";

function fakeDock(workspace: string) {
  const panels: { view: { contentComponent: string } }[] = [];
  const api = {
    panels,
    groups: [],
    addPanel: (o: { component: string }) =>
      panels.push({ view: { contentComponent: o.component } }),
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

  it("reuses a loading pane, and only the latest action runs", () => {
    const panels = fakeDock("c");
    const got: string[] = [];
    inReviewPane("c", () => got.push("first"));
    inReviewPane("c", () => got.push("second"));
    expect(panels).toHaveLength(1);
    publishReview("c", review(5));
    expect(got).toEqual(["second"]);
  });
});
