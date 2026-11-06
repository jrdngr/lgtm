import { describe, expect, it } from "vitest";
import { splitInline } from "./mdExtras";

describe("splitInline", () => {
  it("finds highlights, subscripts and superscripts but not lone marks", () => {
    const parts = splitInline("a ==b c== H~2~O x^2^ 1 ~ 2 == 3").map((n) =>
      n.type === "text" ? n.value : `<${n.type}>${n.children?.[0].value}`,
    );
    expect(parts).toEqual(["a ", "<mark>b c", " H", "<sub>2", "O x", "<sup>2", " 1 ~ 2 == 3"]);
  });
});
