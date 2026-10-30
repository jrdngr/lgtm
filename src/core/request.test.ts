import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { requests } from "./request";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("requests", () => {
  it("go to the panel that mounts after they're sent, once", () => {
    const r = requests<string>();
    r.send("pr");
    const first = vi.fn();
    r.listen(first);
    const second = vi.fn();
    r.listen(second);
    expect(first).toHaveBeenCalledWith("pr");
    expect(second).not.toHaveBeenCalled();
  });

  it("are dropped when no panel mounts within a second", () => {
    const r = requests<string>();
    r.send("pr");
    vi.advanceTimersByTime(1001);
    const late = vi.fn();
    r.listen(late);
    expect(late).not.toHaveBeenCalled();
  });

  it("aren't held when a panel already listens", () => {
    const r = requests<string>();
    const open = vi.fn();
    const stop = r.listen(open);
    r.send("pr");
    stop();
    const next = vi.fn();
    r.listen(next);
    expect(open).toHaveBeenCalledWith("pr");
    expect(next).not.toHaveBeenCalled();
  });
});
