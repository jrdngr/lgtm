import { describe, expect, it, vi } from "vitest";
import type { AgentHookState } from "../bindings";
import { registerTerminal } from "../terminal/registry";
import { mirror, startSync, syncMarker, useSyncedChat } from "./syncedChat";

const added: [string, string][] = [];
vi.mock("../ipc", () => ({
  commands: {
    addChatMessage: (_review: number, _comment: number, author: string, text: string) => {
      added.push([author, text]);
      return Promise.resolve({ status: "ok", data: null });
    },
  },
}));
vi.mock("../terminal/agentHooks", () => ({ watchAgentHooks: () => () => {} }));
vi.mock("react", async (actual) => ({
  ...(await actual<typeof import("react")>()),
  useSyncExternalStore: (_: unknown, get: () => unknown) => get(),
}));

const target = {
  workspace: null,
  title: () => "",
  canPaste: () => true,
  paste() {},
  hasAgent: () => true,
  submit() {},
  focus() {},
};
const report = (
  terminal: string,
  state: AgentHookState,
  said: { prompt?: string; reply?: string },
) => mirror({ terminal, state, prompt: said.prompt ?? null, reply: said.reply ?? null });

describe("synced chat", () => {
  it("mirrors the synced terminal's prompts and replies into its thread until it ends", () => {
    const close = registerTerminal("t1", target);
    registerTerminal("t2", target);
    startSync("t1", 1, 7);

    report("t1", "working", { prompt: `Start. This chat is synced with ${syncMarker(7)}.` });
    report("t1", "working", { prompt: "Is that true?" });
    report("t2", "working", { prompt: "elsewhere" });
    expect(useSyncedChat(7)).toEqual({ synced: true, working: true });
    expect(useSyncedChat(8).synced).toBe(false);
    report("t1", "done", { reply: "Yes." });
    expect(useSyncedChat(7)).toEqual({ synced: true, working: false });
    expect(added).toEqual([
      ["user", "Is that true?"],
      ["agent", "Yes."],
    ]);

    close();
    expect(useSyncedChat(7).synced).toBe(false);
  });

  it("ends when the agent leaves", () => {
    registerTerminal("t3", target);
    startSync("t3", 1, 9);
    report("t3", "gone", {});
    expect(useSyncedChat(9).synced).toBe(false);
  });
});
