import { describe, expect, it } from "vitest";
import type { Stage, ThreadDto } from "../bindings";
import { byStage, planText, sendPlan } from "./stages";

function thread(id: number, stage: Stage, body = "", gh = true, path = "a.rs"): ThreadDto {
  return {
    id,
    path,
    line: 1,
    body,
    state: { repo: "o/n", pr: 1, stage, take: null, summary: "" },
    github: gh
      ? {
          repo: "o/n",
          pr: 1,
          thread: `T${id}`,
          url: "",
          resolved: false,
          outdated: false,
          posts: [],
          line: 1,
        }
      : null,
    snippet: "",
    messages: [],
    placed: true,
    createdAt: 0,
  };
}

describe("sendPlan", () => {
  const threads = [
    thread(1, "ready", "because"),
    thread(2, "ready", "", false), // an empty draft sends nothing
    thread(3, "ready", "new", false),
    thread(4, "ready"), // fixed, on code
    thread(5, "ready", "", true, ""), // fixed, on the PR itself
    thread(6, "reply", "not final yet"),
  ];

  it("counts replies, drafts and fixed threads among the ready ones", () => {
    const p = sendPlan(threads, "leave");
    expect([p.replies, p.drafts, p.fixed, p.doneReplies, p.resolves]).toEqual([1, 1, 2, 0, 0]);
    expect(planText(p)).toBe("1 reply, 1 new comment");
  });

  it("replies Done to fixed threads, and resolves only review threads", () => {
    const p = sendPlan(threads, "replyResolve");
    expect([p.doneReplies, p.resolves]).toEqual([2, 1]);
    expect(planText(p)).toBe("3 replies, 1 new comment, resolve 1");
  });
});

describe("byStage", () => {
  it("orders stages for work and drops empty ones", () => {
    const got = byStage([thread(1, "done"), thread(2, "toFix"), thread(3, "new")]);
    expect(got.map(([s, ts]) => [s, ts.map((t) => t.id)])).toEqual([
      ["new", [3]],
      ["toFix", [2]],
      ["done", [1]],
    ]);
  });
});
