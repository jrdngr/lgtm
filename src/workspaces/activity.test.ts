import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { activity, pulse, report, STALL_MS, selectActivity, tabState } from "./activity";

const stays = { stays: true };

/** The workspace's top state and its agents' labels and states, most urgent first. */
const shown = (ws: string) => {
  const a = activity(ws);
  return { state: a.state, agents: a.agents.map((g) => `${g.label}:${g.state}`) };
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("workspace activity", () => {
  it("lists agents most urgent first, under the most urgent state", () => {
    selectActivity("other");
    report("a", "1", "running", "Ask");
    report("a", "2", "idle", "one", stays);
    expect(shown("a")).toEqual({ state: "working", agents: ["Ask:working", "one:idle"] });
    report("a", "3", "input", "two", stays);
    expect(shown("a").agents).toEqual(["two:input", "Ask:working", "one:idle"]);
    report("a", "1", "failed", "Ask");
    report("a", "3", "running", "two", stays);
    expect(shown("a")).toEqual({
      state: "failed",
      agents: ["Ask:failed", "two:working", "one:idle"],
    });
  });

  it("keeps when an agent took its state across repeated reports", () => {
    selectActivity("other");
    report("t", "1", "running", "Ask");
    vi.advanceTimersByTime(5_000);
    report("t", "1", "running", "Ask");
    pulse("1");
    expect(Date.now() - activity("t").agents[0].since).toBe(5_000);
    report("t", "1", "input", "Ask");
    expect(activity("t").agents[0].since).toBe(Date.now());
  });

  it("gives a tab the most urgent state of the agents in it", () => {
    selectActivity("other");
    report("u", "1", "idle", "one", { stays: true, tab: "term-1" });
    report("u", "2", "input", "two", { stays: true, tab: "term-1" });
    const a = activity("u");
    expect(tabState(a, "term-1")).toBe("input");
    expect(tabState(a, "term-2")).toBeUndefined();
  });

  it("keeps endings until the workspace is seen, then a terminal agent idles and a run goes", () => {
    selectActivity("other");
    report("b", "1", "running", "Change");
    report("b", "2", "running", "Terminal", stays);
    report("b", "1", "done", "Change");
    report("b", "2", "done", "Terminal", stays);
    expect(shown("b")).toEqual({ state: "done", agents: ["Change:done", "Terminal:done"] });
    selectActivity("b");
    expect(shown("b")).toEqual({ state: "idle", agents: ["Terminal:idle"] });
  });

  it("counts endings in the selected workspace as seen, and ignores repeated endings", () => {
    selectActivity("c");
    report("c", "1", "running", "Ask");
    report("c", "1", "done", "Ask");
    selectActivity("other");
    report("c", "1", "done", "Ask");
    report("c", "2", "running", "Reports");
    report("c", "2", "stopped", "Reports");
    expect(activity("c")).toEqual({ state: "none", agents: [] });
  });

  it("calls a running agent stalled after a silence, until it shows signs of life", () => {
    selectActivity("other");
    report("d", "1", "running", "Terminal", stays);
    vi.advanceTimersByTime(STALL_MS + 15_000);
    expect(activity("d").state).toBe("stalled");
    expect(Date.now() - activity("d").agents[0].since).toBe(15_000);
    pulse("1");
    expect(activity("d").state).toBe("working");
    report("d", "1", "input", "Terminal", stays);
    vi.advanceTimersByTime(STALL_MS * 2);
    expect(activity("d").state).toBe("input");
  });
});
