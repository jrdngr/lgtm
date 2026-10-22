import { describe, expect, it } from "vitest";
import { fuzzy, parseQuery } from "./pickStore";

describe("parseQuery", () => {
  it("reads symbols, lines and file lines", () => {
    expect(parseQuery("@ bookm")).toEqual({ kind: "symbols", text: "bookm" });
    expect(parseQuery(":42")).toEqual({ kind: "line", line: 42 });
    expect(parseQuery("edit.rs:12")).toEqual({ kind: "files", text: "edit.rs", line: 12 });
    expect(parseQuery("src/a")).toEqual({ kind: "files", text: "src/a" });
  });
});

describe("fuzzy", () => {
  it("matches characters in order and rejects out of order", () => {
    expect(fuzzy("src/revisions/BookmarkPopup.tsx", "bmp")).toBeDefined();
    expect(fuzzy("src/revisions/BookmarkPopup.tsx", "pmb")).toBeUndefined();
  });

  it("marks the file name over the directory", () => {
    const m = fuzzy("src/review/marks.ts", "mark");
    expect(m?.at).toEqual([11, 12, 13, 14]);
  });

  it("ranks a run in the file name over scattered letters", () => {
    const run = fuzzy("src/search/store.ts", "store");
    const scattered = fuzzy("src/settings/totalRevisions.ts", "store");
    expect(run && scattered && run.score > scattered.score).toBe(true);
  });
});
