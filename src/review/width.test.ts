import { describe, expect, it } from "vitest";
import { columns } from "./width";

describe("columns", () => {
  it("stops tabs at the next multiple of four", () => {
    expect(columns("\tx")).toBe(5);
    expect(columns("ab\tx")).toBe(5);
    expect(columns("abcd\tx")).toBe(9);
  });
});
