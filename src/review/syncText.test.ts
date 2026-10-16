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
  });
});
