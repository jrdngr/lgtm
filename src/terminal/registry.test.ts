import { describe, expect, it } from "vitest";
import {
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
    pinTarget("w", "pb");
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
    pinTarget("w2", "qa");
    expect(sendTarget("w2")).toBe("qb");
    for (const f of off) f();
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
