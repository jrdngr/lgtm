import { describe, expect, it, vi } from "vitest";
import type { CommentDto } from "../bindings";
import { syncText } from "./syncText";

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
});
