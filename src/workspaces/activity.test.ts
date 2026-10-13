import { describe, expect, it } from "vitest";
import { activity, report, selectActivity } from "./activity";

describe("workspace activity", () => {
  it("shows a run that finished while away over one still working, until selected", () => {
    selectActivity("other");
    report("a", "1", "running", "Ask");
    report("a", "2", "running", "Change");
    expect(activity("a")).toEqual({ kind: "working", labels: ["Ask", "Change"] });
    report("a", "1", "done", "Ask");
    expect(activity("a")).toEqual({ kind: "finished", failed: false, labels: ["Ask"] });
    selectActivity("a");
    expect(activity("a")).toEqual({ kind: "working", labels: ["Change"] });
  });

  it("keeps a failure red when a later run finishes fine", () => {
    selectActivity("other");
    report("b", "1", "running", "Triage");
    report("b", "2", "running", "Walkthrough");
    report("b", "1", "failed", "Triage");
    report("b", "2", "done", "Walkthrough");
    expect(activity("b")).toEqual({
      kind: "finished",
      failed: true,
      labels: ["Triage", "Walkthrough"],
    });
  });

  it("ignores runs that end in the selected workspace, stopped runs, and repeated endings", () => {
    selectActivity("c");
    report("c", "1", "running", "Ask");
    report("c", "1", "done", "Ask");
    selectActivity("other");
    report("c", "1", "done", "Ask");
    report("c", "2", "running", "Terminal");
    report("c", "2", "stopped", "Terminal");
    expect(activity("c")).toBeUndefined();
  });
});
