import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { coalesce } from "./coalesce";

describe("coalesce", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("fires once after a burst goes quiet", () => {
    const fire = vi.fn();
    const call = coalesce(fire, 250, 1000);
    call(1);
    vi.advanceTimersByTime(100);
    call(2);
    call(3);
    vi.advanceTimersByTime(249);
    expect(fire).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(fire).toHaveBeenCalledTimes(1);
    expect(fire).toHaveBeenCalledWith([1, 2, 3]);
  });

  it("fires during a burst that never goes quiet, by its longest wait", () => {
    const fire = vi.fn();
    const call = coalesce(fire, 250, 1000);
    for (let t = 0; t < 1500; t += 100) {
      call(t);
      vi.advanceTimersByTime(100);
    }
    expect(fire).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(250);
    expect(fire).toHaveBeenCalledTimes(2);
  });
});
