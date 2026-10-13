import { describe, expect, it } from "vitest";
import type { PostDto } from "../bindings";
import { verdicts } from "./people";

const post = (author: string, verdict: string | null): PostDto => ({
  author,
  body: "",
  verdict,
  createdAt: "",
  url: "",
});

describe("verdicts", () => {
  it("keeps an approval or change request over later plain reviews", () => {
    const posts = [
      post("alice", "APPROVED"),
      post("bob", "COMMENTED"),
      post("alice", "COMMENTED"),
      post("bob", "CHANGES_REQUESTED"),
      post("carol", null),
    ];
    expect(verdicts(posts)).toEqual([
      ["alice", "approved"],
      ["bob", "changes"],
    ]);
  });

  it("drops a dismissed verdict back to commented, until the next one", () => {
    expect(verdicts([post("alice", "APPROVED"), post("alice", "DISMISSED")])).toEqual([
      ["alice", "commented"],
    ]);
    expect(
      verdicts([
        post("bob", "CHANGES_REQUESTED"),
        post("bob", "DISMISSED"),
        post("bob", "APPROVED"),
      ]),
    ).toEqual([["bob", "approved"]]);
  });
});
