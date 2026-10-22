import { describe, expect, it } from "vitest";
import { wordAt } from "./symbolAt";

describe("wordAt", () => {
  it("takes the identifier around the offset, from either edge", () => {
    expect(wordAt("self.jj.act(&action)", 9)).toBe("act");
    expect(wordAt("self.jj.act(&action)", 8)).toBe("act");
    expect(wordAt("self.jj.act(&action)", 11)).toBe("act");
  });

  it("finds nothing on punctuation, space or a number", () => {
    expect(wordAt("a + b", 2)).toBe("");
    expect(wordAt("x = 42", 5)).toBe("");
  });
});
