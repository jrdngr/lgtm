import { describe, expect, it } from "vitest";
import { report } from "../workspaces/activity";
import { claimSend, sending, sentInBackground } from "./sending";

const ws = { repo: "/r", name: "default" } as never;

describe("sending", () => {
  it("spins the clicked button until the terminal's agent finishes its turn", () => {
    claimSend("a");
    sentInBackground("t1");
    const at = Date.now();
    expect(sending("a", at)).toBe(true);
    report(ws, "term:t1", "running", "Claude", { stays: true, tab: "t1" });
    expect(sending("a", at + 60_000)).toBe(true);
    report(ws, "term:t1", "stopped", "Claude", { stays: true, tab: "t1" });
    expect(sending("a", at + 60_000)).toBe(false);
  });

  it("stops once a send no hook reports on has had time to land", () => {
    claimSend("b");
    sentInBackground("t2");
    expect(sending("b", Date.now() + 16_000)).toBe(false);
  });

  it("leaves buttons alone for a send no click claimed", () => {
    sentInBackground("t3");
    expect(sending("c")).toBe(false);
  });
});
