import { describe, expect, it } from "vitest";
import { urlsIn } from "./urlLinks";

describe("urlsIn", () => {
  it("spans each address, leaving out the punctuation after it", () => {
    const text = "  ➜  Local:   http://localhost:5173/ (see https://a.dev/x?y=1).";
    expect(urlsIn(text).map((u) => [u.url, text.slice(u.from, u.to)])).toEqual([
      ["http://localhost:5173/", "http://localhost:5173/"],
      ["https://a.dev/x?y=1", "https://a.dev/x?y=1"],
    ]);
  });
});
