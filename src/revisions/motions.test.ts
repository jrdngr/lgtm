import { describe, expect, it } from "vitest";
import { motions, narrow, step } from "./motions";

const groups = motions("main");

describe("motions", () => {
  it("runs a one-key motion at once and waits on a prefix for the rest", () => {
    expect(step(groups, "", "s")).toMatchObject({ run: { keys: "s" } });
    expect(step(groups, "", "g")).toEqual({ typed: "g" });
    expect(step(groups, "g", "p")).toMatchObject({ run: { keys: "gp" } });
    expect(step(groups, "g", "x")).toBeUndefined();
  });

  it("tells undo from redo by case", () => {
    expect(step(groups, "", "U")).toMatchObject({ run: { label: "Redo" } });
  });

  it("narrows to the prefix's group", () => {
    expect(
      narrow(groups, "n")
        .flat()
        .map((m) => m.keys),
    ).toEqual(["na", "nb"]);
  });

  it("acts on the cursor, squashing the picked range", () => {
    const t = { cursor: "kp", picked: "kp::ml" };
    const by = (k: string) => groups.flat().find((m) => m.keys === k);
    expect(by("nb")?.action(t)).toEqual({ kind: "newBefore", rev: "kp" });
    expect(by("rm")?.action(t)).toEqual({ kind: "rebaseOnTrunk", rev: "kp" });
    expect(by("s")?.action(t)).toEqual({ kind: "squash", revs: "kp::ml" });
    expect(by("rm")?.label).toBe("Rebase onto main@origin");
  });
});
