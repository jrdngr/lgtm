import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentHookState } from "../bindings";
import { registerTerminal } from "../terminal/registry";
import {
  linkAgent,
  linkedTerminal,
  mirror,
  notePasted,
  setCurrentThread,
  unlinkAgent,
  useLink,
} from "./linkedAgent";

const added: [number, string, string][] = [];
const links: [string, boolean, string | null, string | null][] = [];
vi.mock("../ipc", () => ({
  commands: {
    addChatMessage: (_review: number, comment: number, author: string, text: string) => {
      added.push([comment, author, text]);
      return Promise.resolve({ status: "ok", data: null });
    },
    setAgentLink: (
      terminal: string,
      linked: boolean,
      brief: string | null,
      thread: { short: string } | null,
    ) => {
      links.push([terminal, linked, brief, thread?.short ?? null]);
      return Promise.resolve({ status: "ok", data: null });
    },
  },
}));
vi.mock("../terminal/agentHooks", () => ({ watchAgentHooks: () => () => {} }));
vi.mock("./current", () => ({
  currentReview: () => ({
    id: 1,
    files: [],
    comments: [],
    threads: [7, 8].map((id) => ({ id, path: "", line: null, state: {}, github: null })),
  }),
  onReviewChange: () => () => {},
}));
vi.mock("react", async (actual) => ({
  ...(await actual<typeof import("react")>()),
  useSyncExternalStore: (_: unknown, get: () => unknown) => get(),
}));

const target = {
  workspace: "pr",
  title: () => "claude",
  canPaste: () => true,
  paste() {},
  hasAgent: () => true,
  agentBusy: () => false,
  prompted() {},
  submit() {},
  interrupt() {},
  focus() {},
};
const report = (
  terminal: string,
  state: AgentHookState,
  said: { prompt?: string; reply?: string },
) =>
  mirror({
    terminal,
    state,
    prompt: said.prompt ?? null,
    reply: said.reply ?? null,
    session: null,
    ended: false,
    title: null,
  });

beforeEach(() => {
  added.length = 0;
  links.length = 0;
});

describe("linked agent", () => {
  it("files each exchange in the thread I was on when I sent the prompt", () => {
    registerTerminal("t1", target);
    linkAgent("pr", "t1", "brief");
    setCurrentThread("pr", 7);
    report("t1", "working", { prompt: "agree?" });
    expect(useLink("pr")).toMatchObject({ terminal: "t1", title: "claude", working: true });
    setCurrentThread("pr", 8);
    report("t1", "done", { reply: "No." });
    notePasted("t1", 7);
    report("t1", "working", { prompt: "Triage this." });
    report("t1", "done", { reply: "Done." });
    notePasted("t1", null);
    report("t1", "working", { prompt: "Triage them all." });
    report("t1", "done", { reply: "All done." });
    expect(added).toEqual([
      [7, "user", "agree?"],
      [7, "agent", "No."],
      [7, "agent", "Done."],
    ]);
    expect(links.at(-1)).toEqual(["t1", true, "brief", "lgtm: I'm on thread 8 on the PR."]);
  });

  it("moves to another terminal by unlinking the first, and ends when the agent leaves", () => {
    registerTerminal("t2", target);
    registerTerminal("t3", target);
    linkAgent("pr", "t2", "brief");
    linkAgent("pr", "t3", "brief");
    expect(links).toContainEqual(["t2", false, null, null]);
    expect(linkedTerminal("pr")).toBe("t3");
    report("t2", "working", { prompt: "elsewhere" });
    expect(added).toEqual([]);
    unlinkAgent("pr");
    expect(links.at(-1)).toEqual(["t3", false, null, null]);
    linkAgent("pr", "t3", "brief");
    report("t3", "gone", {});
    expect(linkedTerminal("pr")).toBeUndefined();
  });
});
