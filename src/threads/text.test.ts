import { describe, expect, it } from "vitest";
import type { GhPost, Stage, ThreadDto } from "../bindings";
import { finalText, unanswered } from "./text";

const post = (author: string): GhPost => ({ author, body: author, createdAt: "", pending: false });

describe("unanswered", () => {
  it("splits at the viewer's last post", () => {
    const [before, after] = unanswered([post("bob"), post("me"), post("bob"), post("ann")], "me");
    expect(before.map((p) => p.author)).toEqual(["bob", "me"]);
    expect(after.map((p) => p.author)).toEqual(["bob", "ann"]);
  });

  it("shows the last post when the viewer had the last word", () => {
    const [before, after] = unanswered([post("bob"), post("me")], "me");
    expect([before.length, after.map((p) => p.author)]).toEqual([1, ["me"]]);
  });
});

describe("finalText", () => {
  const t = (id: number, stage: Stage, body: string): ThreadDto => ({
    id,
    path: "a.rs",
    line: id,
    body,
    snippet: "",
    messages: [],
    suggestion: null,
    state: { repo: "o/n", pr: 1, stage, take: null, summary: `s${id}` },
    github: {
      repo: "o/n",
      pr: 1,
      thread: "T",
      url: `u${id}`,
      resolved: false,
      outdated: false,
      posts: [post("bob")],
      line: id,
    },
    placed: true,
    fixes: [],
    createdAt: 0,
  });

  it("lays out final replies, fixed threads and what's still open", () => {
    expect(
      finalText([t(1, "ready", "Yes."), t(2, "ready", ""), t(3, "reply", "x"), t(4, "done", "")]),
    ).toBe(
      [
        "Final replies",
        "",
        'a.rs:1: s1\n- Comment: "bob"\n- Reply (final): "Yes."\n- u1',
        "",
        "Fixed as asked",
        "",
        'a.rs:2: s2\n- Comment: "bob"\n- u2',
        "",
        "Still open",
        "",
        "a.rs:3: s3",
      ].join("\n"),
    );
  });
});
