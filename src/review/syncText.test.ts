import { describe, expect, it, vi } from "vitest";
import type { CommentDto } from "../bindings";
import { reviewText, syncText, triageText } from "./syncText";

vi.mock("../terminal/agentHooks", () => ({ watchAgentHooks: () => () => {} }));

const post = (author: string, body: string) => ({ author, body, createdAt: "", pending: false });

describe("syncText", () => {
  it("gives the agent the thread, the reply so far and the thread id to suggest a reply to", () => {
    const c = {
      id: 12,
      path: "",
      body: "Thanks, looking into it.",
      messages: [],
      github: { posts: [post("mkosta", "Won't this refresh twice?")] },
      thread: { quote: null },
    } as unknown as CommentDto;
    const text = syncText([], c);
    expect(text).toContain("mkosta wrote:\nWon't this refresh twice?");
    expect(text).toContain("My reply so far:\nThanks, looking into it.");
    expect(text).toContain("suggest_reply tool with thread_id 12");
    expect(text).toContain("lgtm thread 12");
    expect(text).toContain("Start by telling me what you make of it.");
    expect(text).toContain("set_thread_take tool with thread_id 12");
  });

  it("gives a triaged thread's take instead of asking for one", () => {
    const c = {
      id: 3,
      path: "",
      body: "",
      messages: [],
      github: { posts: [post("mkosta", "Rename this?")] },
      thread: {
        quote: null,
        take: { verdict: "agree", why: "The name hides the unit.", question: null },
      },
    } as unknown as CommentDto;
    const text = syncText([], c);
    expect(text).toContain("Your take from triage (agree):\nThe name hides the unit.");
    expect(text).not.toContain("what you make of it");
  });

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
