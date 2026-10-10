import { describe, expect, it } from "vitest";
import type { CommentDto } from "../bindings";
import { commentSlots, draftFor, followDraft, replyParts } from "./comments";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, flatten, hunkRange, NEW_COMMENT } from "./model";

const files = flatten(review([file("a.rs", [rows([" a", "-b", "+B", " c", "+d"])])]));

describe("comment blocks", () => {
  it("sit after the line holding their last row, inside its hunk", () => {
    const draft = draftFor(files, buildLines(files, layout()), 2, 4);
    expect(draft).toMatchObject({ file: 0, rows: [1, 2] });
    const lines = buildLines(files, layout({ comments: commentSlots([], draft) }));
    expect(lines.map((l) => l.kind)).toEqual([
      "file",
      "hunk",
      "rows",
      "rows",
      "rows",
      "comment",
      "rows",
      "rows",
    ]);
    expect(lines[5]).toMatchObject({ comment: NEW_COMMENT });
    expect(hunkRange(lines, 0, 0)).toEqual([1, 7]);
  });

  it("need a changed row", () => {
    expect(draftFor(files, buildLines(files, layout()), 2, 2)).toBeUndefined();
  });
});

describe("replyParts", () => {
  it("splits suggested comments out of a reply", () => {
    const reply = "Try:\n\n```comment\nWhy `x`?\nIt's unused.\n```\n\nOr drop it.";
    expect(replyParts(reply)).toEqual([
      { kind: "text", text: "Try:" },
      { kind: "suggestion", text: "Why `x`?\nIt's unused." },
      { kind: "text", text: "Or drop it." },
    ]);
  });
});

describe("followDraft", () => {
  const draft = { file: 0, rows: [1, 2], snippet: "" };
  const moves = (rows: [string, string][]) => ({ rows: new Map(rows), files: new Map([[0, 1]]) });

  it("moves the draft with its rows and drops rows that are gone", () => {
    expect(followDraft(draft, moves([["0:2", "1:5"]]))).toMatchObject({ file: 1, rows: [5] });
  });

  it("drops the draft when its rows are gone or the review was replaced", () => {
    expect(followDraft(draft, moves([]))).toBeUndefined();
    expect(followDraft(draft, undefined)).toBeUndefined();
  });
});

describe("comments whose code is gone", () => {
  it("sit under their file's header", () => {
    const gone = { id: 7, file: 0, rows: null } as unknown as CommentDto;
    const lines = buildLines(files, layout({ comments: commentSlots([gone], undefined) }));
    expect(lines.slice(0, 3).map((l) => l.kind)).toEqual(["file", "comment", "hunk"]);
  });
});
