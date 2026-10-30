// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const spawned: number[] = [];
const killed: number[] = [];
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("../ipc", () => ({
  commands: {
    ptySpawn: () => {
      spawned.push(spawned.length);
      return Promise.resolve({ status: "ok", data: spawned.length - 1 });
    },
    ptyKill: (id: number) => killed.push(id),
  },
}));
/** The last session's agent hook listener. */
let hook = (_: string) => {};
vi.mock("./agentHooks", () => ({
  onAgentHook: (_: string, l: (state: string) => void) => {
    hook = l;
    return () => {};
  },
}));
vi.mock("./xterm", () => ({
  createTerminal: () => ({
    term: {
      cols: 80,
      rows: 24,
      registerLinkProvider: sub,
      onData: sub,
      onTitleChange,
      onResize,
      dispose() {},
    },
    refit() {},
    release() {},
    fontsReady: Promise.resolve(),
  }),
}));
const sub = () => ({ dispose() {} });
/** The last session's title listener. */
let setTitle = (_: string) => {};
const onTitleChange = (l: (t: string) => void) => {
  setTitle = l;
  return sub();
};

/** The last session's resize listeners. */
let resizes: ((size: { cols: number; rows: number }) => void)[] = [];
const onResize = (l: (size: { cols: number; rows: number }) => void) => {
  resizes.push(l);
  return { dispose: () => (resizes = resizes.filter((r) => r !== l)) };
};

const { openSession, releaseSession } = await import("./sessions");
const box = () => document.body.appendChild(document.createElement("div"));
const settle = () => vi.advanceTimersByTimeAsync(250);

beforeEach(() => {
  vi.useFakeTimers();
  spawned.length = 0;
  killed.length = 0;
});
afterEach(() => vi.useRealTimers());

describe("terminal sessions", () => {
  it("keep the shell when the panel mounts in the other dock before the old one unmounts", async () => {
    const before = openSession("t1", box(), null, false);
    const target = box();
    const after = openSession("t1", target, null, false);
    releaseSession("t1");
    await vi.advanceTimersByTimeAsync(5000);
    expect(after).toBe(before);
    expect(target.contains(after.host)).toBe(true);
    expect(spawned).toHaveLength(1);
    expect(killed).toEqual([]);
  });

  it("start the shell once the terminal has stopped resizing", async () => {
    resizes = [];
    openSession("t8", box(), null, false);
    await vi.advanceTimersByTimeAsync(200);
    for (const r of resizes) r({ cols: 100, rows: 30 });
    await vi.advanceTimersByTimeAsync(200);
    expect(spawned).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(50);
    expect(spawned).toHaveLength(1);
  });

  it("keep the shell when the panel unmounts before it mounts again", async () => {
    const before = openSession("t2", box(), null, false);
    await settle();
    releaseSession("t2");
    expect(openSession("t2", box(), null, false)).toBe(before);
    await vi.advanceTimersByTimeAsync(5000);
    expect(killed).toEqual([]);
  });

  it("kill the shell once its panel closes for good", async () => {
    openSession("t3", box(), null, false);
    await settle();
    releaseSession("t3");
    await vi.advanceTimersByTimeAsync(5000);
    expect(killed).toEqual([0]);
  });

  it("trust an agent title only in a terminal started to run Claude Code", async () => {
    const shell = openSession("t4", box(), null, false);
    await settle();
    setTitle("\u2802 spoofed");
    expect(shell.hasAgent()).toBe(false);
    const claude = openSession("t5", box(), null, false, Promise.resolve("claude"));
    await settle();
    setTitle("\u2802 working");
    expect(claude.hasAgent()).toBe(true);
  });

  it("count as busy once prompted, until the prompt has had time to land", async () => {
    const s = openSession("t6", box(), null, false);
    await settle();
    expect(s.agentBusy()).toBe(false);
    s.prompted();
    expect(s.agentBusy()).toBe(true);
    await vi.advanceTimersByTimeAsync(15_000);
    expect(s.agentBusy()).toBe(false);
  });

  it("stop counting a working agent as busy once it has written nothing for a while", async () => {
    const s = openSession("t7", box(), null, false);
    await settle();
    hook("working");
    await vi.advanceTimersByTimeAsync(19_000);
    expect(s.agentBusy()).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.agentBusy()).toBe(false);
    hook("input");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(s.agentBusy()).toBe(true);
  });
});
