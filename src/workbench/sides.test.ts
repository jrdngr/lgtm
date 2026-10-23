import { describe, expect, it } from "vitest";
import { keepRightView, rightViewOf, type Sides } from "./sides";

const sides = (view: string): Sides => ({
  left: { open: true, width: 230, view: "workspaces" },
  right: { open: true, width: 260, view },
});

describe("right panel views per workspace", () => {
  it("brings back each workspace's view and leaves one with none on the current view", () => {
    let s = keepRightView(sides("guided"), "a");
    s = keepRightView({ ...s, right: { ...s.right, view: "changes" } }, "b");
    expect(rightViewOf(s, "a").right.view).toBe("guided");
    expect(rightViewOf(s, "b").right.view).toBe("changes");
    expect(rightViewOf(s, "new").right.view).toBe("changes");
  });
});
