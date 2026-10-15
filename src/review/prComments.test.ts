import { describe, expect, it } from "vitest";
import type { CommentDto, ThreadDto } from "../bindings";
import { prComments } from "./prComments";

const thread = (id: number, path: string): ThreadDto => ({
  id,
  path,
  line: path ? 3 : null,
  body: "",
  state: { repo: "o/n", pr: 1, stage: "new", take: null, summary: "" },
  github: {
    repo: "o/n",
    pr: 1,
    thread: `T${id}`,
    url: "",
    resolved: false,
    outdated: false,
    posts: [],
    line: path ? 3 : null,
  },
  snippet: "old code",
  messages: [],
  placed: false,
  createdAt: id,
});

const inReview = (id: number) => ({ id }) as CommentDto;

describe("prComments", () => {
  it("adds threads on files the review doesn't show after the comments on the PR", () => {
    const threads = [thread(1, "gone.rs"), thread(2, ""), thread(3, "kept.rs")];
    const blocks = prComments(threads, [inReview(3)]);
    expect(blocks.map((c) => [c.id, c.path, c.file])).toEqual([
      [2, "", -1],
      [1, "gone.rs", -1],
    ]);
    expect(blocks[1].snippet).toBe("old code");
  });
});
