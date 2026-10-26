import { describe, expect, it, vi } from "vitest";
import {
  keepSubmit,
  pinTarget,
  promptAgent,
  registerTerminal,
  sendTarget,
  sendToTerminal,
  submitNextSend,
  touchTerminal,
  workspaceTerminals,
} from "./registry";

function fake(workspace: string | null, canPaste = true, agent = false) {
  const got: string[] = [];
  return {
    got,
    target: {
      hasAgent: () => agent,
      agentBusy: () => false,
      prompted() {},
      interrupt() {},
      submit: () => got.push("\r"),
      workspace,
      title: () => "",
      canPaste: () => canPaste,
      paste: (t: string) => got.push(t),
      focus() {},
    },
  };
}

describe("sendToTerminal", () => {
  it("pastes into the workspace's send target, or a chosen terminal", () => {
    const a = fake("t1");
    const b = fake("t1");
    const main = fake(null);
    const off = [registerTerminal("a", a.target), registerTerminal("b", b.target)];
    off.push(registerTerminal("m", main.target));
    touchTerminal("a");
    expect(sendToTerminal("t1", "x")).toBe("sent");
    expect(a.got).toEqual(["x"]);
    expect(b.got).toEqual([]);
    expect(workspaceTerminals("t1").map((t) => t.id)).toEqual(["a", "b"]);
    expect(sendToTerminal("t1", "y", "b")).toBe("sent");
    expect(b.got).toEqual(["y"]);
    expect(sendToTerminal("other", "x")).toBe("noTerminal");
    for (const f of off) f();
    expect(sendToTerminal("t1", "x")).toBe("noTerminal");
  });

  it("refuses a terminal without bracketed paste", () => {
    const shell = fake(null, false);
    const off = registerTerminal("s", shell.target);
    expect(sendToTerminal(null, "rm -rf x")).toBe("unsafe");
    expect(shell.got).toEqual([]);
    off();
  });

  it("strips control characters that could end bracketed paste", () => {
    const t = fake(null);
    const off = registerTerminal("t", t.target);
    expect(sendToTerminal(null, "a\x1b[201~rm -rf x\n\tb\r")).toBe("sent");
    expect(t.got).toEqual(["a[201~rm -rf x\n\tb"]);
    off();
  });
});

describe("send target", () => {
  it("is the first opened terminal until another is pinned, whatever was focused", () => {
    const a = fake("w");
    const b = fake("w");
    const c = fake("w");
    const off = [registerTerminal("pa", a.target), registerTerminal("pb", b.target)];
    touchTerminal("pb");
    expect(sendTarget("w")).toBe("pa");
    pinTarget("w", "pb", "interactive");
    off.push(registerTerminal("pc", c.target));
    touchTerminal("pc");
    expect(sendTarget("w")).toBe("pb");
    expect(workspaceTerminals("w").map((t) => t.id)).toEqual(["pb", "pc", "pa"]);
    expect(sendToTerminal("w", "x")).toBe("sent");
    expect(b.got).toEqual(["x"]);
    off[1]();
    expect(sendTarget("w")).toBe("pa");
    off[0]();
    expect(sendTarget("w")).toBe("pc");
    off[2]();
    expect(sendTarget("w")).toBeUndefined();
  });

  it("ignores a pin on another workspace's terminal", () => {
    const a = fake("w1");
    const b = fake("w2");
    const off = [registerTerminal("qa", a.target), registerTerminal("qb", b.target)];
    pinTarget("w2", "qa", "interactive");
    expect(sendTarget("w2")).toBe("qb");
    for (const f of off) f();
  });

  it("sends background text only to the pinned background terminal, without focusing it", () => {
    const a = fake("bw");
    const b = fake("bw");
    const focused: string[] = [];
    const off = [
      registerTerminal("ba", { ...a.target, focus: () => focused.push("ba") }),
      registerTerminal("bb", { ...b.target, focus: () => focused.push("bb") }),
    ];
    expect(sendTarget("bw", "background")).toBeUndefined();
    pinTarget("bw", "ba", "background");
    expect(sendTarget("bw", "background")).toBe("ba");
    expect(sendTarget("bw")).toBe("bb");
    expect(sendToTerminal("bw", "triage", undefined, "background")).toBe("sent");
    expect(a.got).toEqual(["triage"]);
    expect(focused).toEqual([]);
    for (const f of off) f();
  });

  it("moves a terminal's pin when it's pinned for the other role", () => {
    const a = fake("mw");
    const off = registerTerminal("ma", a.target);
    pinTarget("mw", "ma", "background");
    pinTarget("mw", "ma", "interactive");
    expect(sendTarget("mw", "background")).toBeUndefined();
    expect(sendTarget("mw")).toBe("ma");
    off();
  });
});

describe("submitNextSend", () => {
  it("presses Enter after the next send only when an agent runs in the target", () => {
    const agent = fake("s1", true, true);
    const off = registerTerminal("s-agent", agent.target);
    submitNextSend();
    sendToTerminal("s1", "fix it");
    sendToTerminal("s1", "plain");
    off();
    expect(agent.got).toEqual(["fix it", "\r", "plain"]);

    const shell = fake("s2");
    const off2 = registerTerminal("s-shell", shell.target);
    submitNextSend();
    sendToTerminal("s2", "ls");
    off2();
    expect(shell.got).toEqual(["ls"]);
  });
});

describe("keepSubmit", () => {
  it("keeps a send armed past the window while its text is worked out", async () => {
    vi.useFakeTimers();
    const agent = fake("k1", true, true);
    const off = registerTerminal("k-agent", agent.target);
    let resolve = (_: string) => {};
    const text = new Promise<string>((r) => {
      resolve = r;
    });
    submitNextSend();
    const kept = keepSubmit(text);
    vi.advanceTimersByTime(10_000);
    resolve("describe it");
    sendToTerminal("k1", await kept);
    off();
    vi.useRealTimers();
    expect(agent.got).toEqual(["describe it", "\r"]);
  });

  it("doesn't arm a send that wasn't armed", async () => {
    const agent = fake("k2", true, true);
    const off = registerTerminal("k-agent2", agent.target);
    sendToTerminal("k2", await keepSubmit(Promise.resolve("plain")));
    off();
    expect(agent.got).toEqual(["plain"]);
  });
});

describe("promptAgent", () => {
  it("submits only to a terminal running an agent", () => {
    const shell = fake("w", true, false);
    const agent = fake("w", true, true);
    const off = [registerTerminal("s", shell.target), registerTerminal("a", agent.target)];
    expect(promptAgent("s", "go")).toBe(false);
    expect(shell.got).toEqual([]);
    expect(promptAgent("a", "go\x1b")).toBe(true);
    expect(agent.got).toEqual(["go", "\r"]);
    for (const f of off) f();
  });
});
