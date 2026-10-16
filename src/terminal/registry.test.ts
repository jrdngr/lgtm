import { describe, expect, it } from "vitest";
import {
  pinTarget,
  registerTerminal,
  sendTarget,
  sendToTerminal,
  touchTerminal,
  workspaceTerminals,
} from "./registry";

function fake(workspace: string | null, canPaste = true) {
  const got: string[] = [];
  return {
    got,
    target: {
      workspace,
      title: () => "",
      canPaste: () => canPaste,
      paste: (t: string) => got.push(t),
      focus() {},
    },
  };
}

describe("sendToTerminal", () => {
  it("pastes into the workspace's most recently focused terminal", () => {
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
  it("is the pinned terminal while it's open, else the most recently focused", () => {
    const a = fake("w");
    const b = fake("w");
    const off = [registerTerminal("pa", a.target), registerTerminal("pb", b.target)];
    expect(sendTarget("w")).toBe("pb");
    pinTarget("w", "pa");
    touchTerminal("pb");
    expect(sendTarget("w")).toBe("pa");
    expect(workspaceTerminals("w").map((t) => t.id)).toEqual(["pa", "pb"]);
    expect(sendToTerminal("w", "x")).toBe("sent");
    expect(a.got).toEqual(["x"]);
    off[0]();
    expect(sendTarget("w")).toBe("pb");
    pinTarget("w", null);
    off[1]();
  });

  it("ignores a pin on another workspace's terminal", () => {
    const a = fake("w1");
    const b = fake("w2");
    const off = [registerTerminal("qa", a.target), registerTerminal("qb", b.target)];
    pinTarget("w2", "qa");
    expect(sendTarget("w2")).toBe("qb");
    pinTarget("w2", null);
    for (const f of off) f();
  });
});
