import { describe, expect, it } from "vitest";
import type { ProgressDto } from "../bindings";
import { doneShare } from "./progress";

const p = (changed: number, left: number): ProgressDto => ({
  revset: "@",
  done: changed - left,
  changed,
  marks: [],
});

describe("doneShare", () => {
  it("stays short of done while a row is left, however many there are", () => {
    expect(Math.round(doneShare(p(1000, 1)) * 100)).toBeLessThan(100);
  });
});
