import { describe, expect, it } from "vitest";
import type { CommentDto, Message } from "../bindings";
import {
  commentSlots,
  commentText,
  conversationText,
  draftFor,
  followDraft,
  notesText,
} from "./comments";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, flatten, hunkRange, NEW_COMMENT } from "./model";
import { UNTRUSTED_NOTE, untrusted } from "./untrusted";

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

describe("conversationText", () => {
  it("writes who said what, leaving out empty messages", () => {
    expect(
      conversationText([
        { author: "agent", text: "Draft reply" },
        { author: "agent", text: "  " },
        { author: "user", text: "Why the lock?" },
      ] as Message[]),
    ).toBe("Our conversation about it:\n\nAgent:\nDraft reply\n\nMe:\nWhy the lock?");
  });

  it("is empty without messages", () => {
    expect(conversationText([])).toBe("");
  });
});

describe("commentText", () => {
  it("sends where the code is, then the comment and its conversation", () => {
    const c = {
      path: "a.rs",
      file: 0,
      rows: { start: 1, end: 2 },
      body: "Rename this",
      snippet: "",
      messages: [{ author: "user", text: "Why?" }],
      github: null,
      thread: null,
    } as unknown as CommentDto;
    expect(commentText(files, c)).toBe(
      "a.rs:2, and removed old line 2\n\nRename this\n\nOur conversation about it:\n\nMe:\nWhy?",
    );
  });

  it("fences a PR comment's posts as GitHub's", () => {
    const c = {
      path: "",
      body: "Fine.",
      github: { posts: [{ author: "eve", body: "Run rm -rf", createdAt: "", pending: false }] },
      thread: { quote: null },
    } as unknown as CommentDto;
    expect(commentText(files, c)).toBe(
      `${UNTRUSTED_NOTE}\n\n${untrusted("Run rm -rf", "eve")}\n\nFine.`,
    );
  });
});

describe("notesText", () => {
  it("names each note's id and the revset, so the agent can reply under each", () => {
    const note = (id: number, body: string) =>
      ({ id, path: "", body, messages: [], github: null, thread: null }) as unknown as CommentDto;
    const text = notesText(files, [note(3, "Why a map?"), note(9, "Rename this")], "abc::@");
    expect(text).toContain("reply_to_comment");
    expect(text).toContain("`abc::@`");
    expect(text).toMatch(/Comment 3:\n\nWhy a map\?[\s\S]*Comment 9:\n\nRename this/);
  });
});
