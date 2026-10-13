import { describe, expect, it } from "vitest";
import type { ProgressDto } from "../bindings";
import { doneShare } from "./progress";

const p = (changed: number, unreviewed: number): ProgressDto => ({
  revset: "@",
  reviewed: changed - unreviewed,
  skipped: 0,
  flagged: 0,
  unreviewed,
  changed,
});

describe("doneShare", () => {
  it("stays short of done while a row is left, however many there are", () => {
    expect(Math.round(doneShare(p(1000, 1)) * 100)).toBeLessThan(100);
  });
});
