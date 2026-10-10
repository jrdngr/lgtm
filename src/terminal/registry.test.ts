import { describe, expect, it } from "vitest";
import { registerTerminal, sendToTerminal, touchTerminal, workspaceTerminals } from "./registry";

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
});
