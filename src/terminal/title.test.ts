import { describe, expect, it } from "vitest";
import { isBusyTitle } from "./title";

describe("isBusyTitle", () => {
  it("reads Claude Code's spinner frames as busy and its idle mark as not", () => {
    expect(isBusyTitle("⠂ Fix anchoring")).toBe(true);
    expect(isBusyTitle("⠐ Fix anchoring")).toBe(true);
    expect(isBusyTitle("✳ Fix anchoring")).toBe(false);
    expect(isBusyTitle("zsh")).toBe(false);
  });
});
