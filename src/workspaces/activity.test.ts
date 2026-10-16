import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { activity, pulse, report, STALL_MS, selectActivity } from "./activity";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("workspace activity", () => {
  it("shows the most urgent agent and counts them all", () => {
    selectActivity("other");
    report("a", "1", "running", "Ask");
    report("a", "2", "idle", "Terminal: one", true);
    expect(activity("a")).toEqual({ state: "working", count: 2, labels: ["Ask"] });
    report("a", "3", "input", "Terminal: two", true);
    expect(activity("a")).toEqual({ state: "input", count: 3, labels: ["Terminal: two"] });
    report("a", "1", "failed", "Ask");
    expect(activity("a").state).toBe("input");
    report("a", "3", "running", "Terminal: two", true);
    expect(activity("a")).toEqual({ state: "failed", count: 3, labels: ["Ask"] });
  });

  it("keeps endings until the workspace is seen, then a terminal agent idles and a run goes", () => {
    selectActivity("other");
    report("b", "1", "running", "Change");
    report("b", "2", "running", "Terminal", true);
    report("b", "1", "done", "Change");
    report("b", "2", "done", "Terminal", true);
    expect(activity("b")).toEqual({ state: "done", count: 2, labels: ["Change", "Terminal"] });
    selectActivity("b");
    expect(activity("b")).toEqual({ state: "idle", count: 1, labels: ["Terminal"] });
  });

  it("counts endings in the selected workspace as seen, and ignores repeated endings", () => {
    selectActivity("c");
    report("c", "1", "running", "Ask");
    report("c", "1", "done", "Ask");
    selectActivity("other");
    report("c", "1", "done", "Ask");
    report("c", "2", "running", "Reports");
    report("c", "2", "stopped", "Reports");
    expect(activity("c")).toEqual({ state: "none", count: 0, labels: [] });
  });

  it("calls a running agent stalled after a silence, until it shows signs of life", () => {
    selectActivity("other");
    report("d", "1", "running", "Terminal", true);
    vi.advanceTimersByTime(STALL_MS + 15_000);
    expect(activity("d").state).toBe("stalled");
    pulse("1");
    expect(activity("d").state).toBe("working");
    report("d", "1", "input", "Terminal", true);
    vi.advanceTimersByTime(STALL_MS * 2);
    expect(activity("d").state).toBe("input");
  });
});
