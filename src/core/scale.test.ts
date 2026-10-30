// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { onScaleSettled } from "./scale";

/** The media query listeners, which a ratio change fires. */
let fire: (() => void)[] = [];

function setRatio(r: number) {
  Object.defineProperty(window, "devicePixelRatio", { value: r, configurable: true });
  const now = fire;
  fire = [];
  for (const f of now) f();
  window.dispatchEvent(new Event("resize"));
}

/** Subscribes, unsubscribing after the test. */
let stops: (() => void)[] = [];
function watch() {
  const seen = vi.fn();
  stops.push(onScaleSettled(seen));
  return seen;
}

beforeEach(() => {
  vi.useFakeTimers();
  fire = [];
  setRatio(2);
  vi.stubGlobal("matchMedia", () => ({
    addEventListener: (_: string, f: () => void) => fire.push(f),
    removeEventListener: (_: string, f: () => void) => {
      fire = fire.filter((g) => g !== f);
    },
  }));
});
afterEach(() => {
  for (const stop of stops) stop();
  stops = [];
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("scale changes", () => {
  it("report once a burst of ratio changes holds still", async () => {
    const seen = watch();
    setRatio(1);
    await vi.advanceTimersByTimeAsync(50);
    setRatio(1.5);
    await vi.advanceTimersByTimeAsync(99);
    expect(seen).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(seen).toHaveBeenCalledTimes(1);
  });

  it("report a zoom that passes through another ratio and lands back where it was", async () => {
    const seen = watch();
    setRatio(1);
    setRatio(2);
    await vi.advanceTimersByTimeAsync(100);
    expect(seen).toHaveBeenCalledTimes(1);
  });

  it("ignore a window resize that leaves the ratio alone", async () => {
    const seen = watch();
    window.dispatchEvent(new Event("resize"));
    await vi.advanceTimersByTimeAsync(100);
    expect(seen).not.toHaveBeenCalled();
  });
});
