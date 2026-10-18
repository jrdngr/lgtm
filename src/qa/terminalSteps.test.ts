// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TerminalAgent } from "../bindings";
import type { Session } from "./model";

const watchers = new Set<(r: TerminalAgent) => void>();
const errors: (string | undefined)[] = [];
vi.mock("../ipc", () => ({ commands: {} }));
vi.mock("../review/linkedAgent", () => ({ notePasted() {} }));
vi.mock("../terminal/agentHooks", () => ({
  watchAgentHooks: (w: (r: TerminalAgent) => void) => {
    watchers.add(w);
    return () => watchers.delete(w);
  },
}));
vi.mock("./store", () => ({
  act: (r: unknown) => r,
  setBusy() {},
  setError: (_: unknown, e?: string) => errors.push(e),
}));

const { registerTerminal } = await import("../terminal/registry");
const { runInTerminal } = await import("./terminalSteps");

/** A terminal with an agent, busy from a prompt until a hook reports, like a real one. */
function agentTerminal(id: string) {
  const pasted: string[] = [];
  let busy = false;
  let agent = true;
  const off = registerTerminal(id, {
    workspace: null,
    title: () => id,
    canPaste: () => true,
    paste: (t) => pasted.push(t),
    hasAgent: () => agent,
    agentBusy: () => busy,
    prompted: () => {
      busy = true;
    },
    submit() {},
    interrupt() {},
    focus() {},
  });
  const report = (state: TerminalAgent["state"], reply?: string) => {
    busy = state === "working" || state === "input";
    for (const w of [...watchers])
      w({ terminal: id, state, reply: reply ?? null } as TerminalAgent);
  };
  return { pasted, report, off, exit: () => (agent = false) };
}

const session = (id: string, terminal: string) =>
  ({ id, title: id, terminal }) as unknown as Session;
const step = (d: Session, prompt: string) =>
  runInTerminal(null, d, { kind: "discuss", about: "q1", prompt });
const settle = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => {
  vi.useFakeTimers();
  errors.length = 0;
  watchers.clear();
});
afterEach(() => vi.useRealTimers());

describe("terminal steps", () => {
  it("send one prompt at a time to a terminal two sessions share", async () => {
    const t = agentTerminal("shared");
    const a = step(session("a", "shared"), "first");
    const b = step(session("b", "shared"), "second");
    await settle();
    expect(t.pasted).toEqual(["first"]);
    t.report("working");
    t.report("done", "reply one");
    expect(await a).toBe("reply one");
    await settle();
    expect(t.pasted).toEqual(["first", "second"]);
    t.report("working");
    t.report("done", "reply two");
    expect(await b).toBe("reply two");
    t.off();
  });

  it("wait while a prompt is out but its hook hasn't come", async () => {
    const t = agentTerminal("slow");
    step(session("c", "slow"), "first");
    await settle();
    // A second terminal step from elsewhere checks the terminal before any hook lands.
    const { terminalAgentBusy } = await import("../terminal/registry");
    expect(terminalAgentBusy("slow")).toBe(true);
    t.off();
  });

  it("end when the agent's terminal goes away mid-turn, freeing the queue", async () => {
    const t = agentTerminal("lost");
    const a = step(session("d", "lost"), "first");
    await settle();
    t.report("working");
    t.exit();
    await vi.advanceTimersByTimeAsync(1000);
    expect(await a).toBeUndefined();
    expect(errors).toEqual(["The agent in lost ended"]);
    t.off();
  });

  it("end when the terminal is closed mid-turn", async () => {
    const t = agentTerminal("closed");
    const a = step(session("e", "closed"), "first");
    await settle();
    t.report("working");
    t.off();
    await vi.advanceTimersByTimeAsync(1000);
    expect(await a).toBeUndefined();
    expect(errors).toEqual(["The agent in closed ended"]);
  });
});
