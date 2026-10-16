import { describe, expect, it } from "vitest";
import { clearFinished, agentRuns as list, startRun } from "./runs";

describe("agent runs", () => {
  it("keeps how a run ended when late activity arrives", () => {
    const h = startRun("late", { kind: "triage", subject: "PR threads" });
    h.end("failed", "boom");
    h.activity("reading a.ts");
    expect(list("late")[0]).toMatchObject({ outcome: "failed", detail: "boom" });
  });

  it("clears finished runs but keeps running ones", () => {
    const done = startRun("clear", { kind: "qa", subject: "a.ts" });
    startRun("clear", { kind: "fix", subject: "b.ts" });
    done.end("done");
    clearFinished("clear");
    expect(list("clear").map((r) => r.kind)).toEqual(["fix"]);
  });
});
