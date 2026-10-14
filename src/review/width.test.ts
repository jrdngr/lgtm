import { describe, expect, it } from "vitest";
import { columns } from "./width";

describe("columns", () => {
  it("stops tabs at the next multiple of four", () => {
    expect(columns("\tx")).toBe(5);
    expect(columns("ab\tx")).toBe(5);
    expect(columns("abcd\tx")).toBe(9);
  });

  it("counts wide characters as two columns", () => {
    expect(columns("漢字")).toBe(4);
    expect(columns("ｘ😀")).toBe(4);
    expect(columns("漢\tx")).toBe(5);
    expect(columns("aé→")).toBe(3);
  });
});
