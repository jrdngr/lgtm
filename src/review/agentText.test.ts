import { describe, expect, it } from "vitest";
import type { CommentDto, PrDto, ThreadDto } from "../bindings";
import { LINK_START, linkText, reviewText, threadContext, triageText } from "./agentText";

const post = (author: string, body: string) => ({ author, body, createdAt: "", pending: false });

describe("linkText", () => {
  it("names the PR, the tools and my rules, and starts so its prompt can be told apart", () => {
    const pr = { number: 482, title: "Batch loads" } as PrDto;
    const text = linkText(pr, "No compliments.\n");
    expect(text.startsWith(LINK_START)).toBe(true);
    expect(text).toContain('#482 "Batch loads"');
    expect(text).toContain("My rules for takes and replies:\n\nNo compliments.");
    expect(text).toContain("suggest_reply");
  });
});

describe("threadContext", () => {
  const t = {
    id: 31,
    path: "src/revset.ts",
    line: 88,
    state: { quote: null },
    github: { posts: [post("mkosta", "Twice?")] },
  } as unknown as ThreadDto;

  it("names the thread in a line, and gives it whole with its posts and my reply", () => {
    const c = {
      id: 31,
      path: "",
      body: "Once.",
      github: t.github,
      thread: { quote: null },
    } as unknown as CommentDto;
    const { short, full } = threadContext([], t, c);
    expect(short).toBe("lgtm: I'm on thread 31, mkosta's on src/revset.ts:88.");
    expect(full).toBe(`${short}\n\nmkosta wrote:\nTwice?\n\nMy reply so far:\nOnce.`);
  });

  it("falls back to the line when the comment isn't loaded, and names PR comments", () => {
    expect(threadContext([], t, undefined).full).toBe(threadContext([], t, undefined).short);
    const onPr = { ...t, path: "", line: null } as ThreadDto;
    expect(threadContext([], onPr, undefined).short).toBe(
      "lgtm: I'm on thread 31, mkosta's on the PR.",
    );
  });
});

describe("agent texts", () => {
  it("has a review hand the agent my reply rules and how to suggest a better reply", () => {
    const c = {
      id: 5,
      path: "",
      body: "Kept it.",
      messages: [],
      github: { posts: [post("mkosta", "Inline this?")] },
      thread: { quote: null },
    } as unknown as CommentDto;
    const text = reviewText([], c, "No compliments.\n");
    expect(text).toContain("mkosta wrote:\nInline this?");
    expect(text).toContain("My reply so far:\nKept it.");
    expect(text).toContain("My rules for takes and replies:\n\nNo compliments.");
    expect(text).toContain("suggest_reply tool with thread_id 5");
  });

  it("has a triage hand over my rules and set the take and reply in one call", () => {
    const c = {
      id: 8,
      path: "",
      body: "",
      messages: [],
      github: { posts: [post("mkosta", "Split this?")] },
      thread: { quote: null, take: null },
    } as unknown as CommentDto;
    const text = triageText([], c, "Agree when they're right.");
    expect(text).toContain("mkosta wrote:\nSplit this?");
    expect(text).toContain("My rules for takes and replies:\n\nAgree when they're right.");
    expect(text).toContain("set_thread_take tool with thread_id 8");
    expect(text).not.toContain("My reply so far");
  });
});
