import { describe, expect, it } from "vitest";
import type { BotRun, Finding, Part } from "../bindings";
import { file, review, rows } from "../review/fixture.test-utils";
import { flatten } from "../review/model";
import {
  actionsText,
  findingText,
  history,
  locationText,
  place,
  refLabel,
  report,
  tally,
} from "./findings";

const finding = (id: string, over: Partial<Finding> = {}): Finding => ({
  id,
  severity: "high",
  title: `Title ${id}`,
  locations: [{ path: "src/a.rs", start: 2, end: 3 }],
  facts: ["a fact"],
  explanation: "Why it matters.",
  fix: "Do this.",
  offLens: false,
  ...over,
});

const done = (over: Partial<Part>): Part => ({
  step: "reports",
  persona: null,
  status: "done",
  output: null,
  error: null,
  session: null,
  ...over,
});

const run = (parts: Part[]): BotRun => ({
  id: 1,
  workspace: null,
  revset: "@",
  from: null,
  to: "c",
  personas: [],
  options: { crossReview: true, suggest: true },
  parts,
  done: [],
  createdAt: 0,
  active: false,
});

describe("report", () => {
  const r = report(
    run([
      done({
        persona: "sec",
        output: { kind: "review", verdict: "no", findings: [finding("sec#1")] },
      }),
      done({
        persona: "simple",
        step: "challenges",
        output: { kind: "challenges", challenges: [{ finding: "sec#1", argument: "overrated" }] },
      }),
      done({
        persona: "sec",
        step: "rebuttals",
        output: {
          kind: "rebuttals",
          responses: [{ finding: "sec#1", stance: "defend", evidence: "see line 2" }],
        },
      }),
      done({
        step: "synthesis",
        output: {
          kind: "synthesis",
          verdict: "fix 1",
          findings: [finding("1", { sources: ["sec#1"] })],
          rulings: [{ finding: "sec#1", outcome: "kept", severity: null, reason: "holds" }],
          quickWins: [],
          questions: [],
          fixFirst: [],
        },
      }),
      done({ persona: "general", status: "failed", output: null }),
    ]),
  );

  it("gathers each layer", () => {
    expect([...r.byPersona.keys()]).toEqual(["sec"]);
    expect(r.synthesis?.verdict).toBe("fix 1");
    expect(r.exchanges.map((e) => `${e.kind}:${e.by}`)).toEqual([
      "challenge:simple",
      "response:sec",
    ]);
  });

  it("puts a merged finding's cross-review history on it", () => {
    const merged = r.synthesis?.findings[0];
    if (!merged) throw new Error("no synthesis");
    const h = history(r, merged);
    expect(h.exchanges).toHaveLength(2);
    expect(h.rulings[0].outcome).toBe("kept");
  });
});

it("shows the newest suggested fixes, which replace the ones from an older task", () => {
  const filter = (task: string): Part =>
    done({ step: "minimal", output: { kind: "minimal", task, fixes: [], offTask: [] } });
  expect(report(run([filter("old"), filter("new")])).minimal?.task).toBe("new");
});

it("numbers a lone persona's findings without its name", () => {
  const lone = {
    ...run([]),
    personas: [{ name: "sec", description: "", offLens: false, profile: "" }],
  };
  expect(refLabel(lone, "sec#2")).toBe("#2");
  expect(refLabel(run([]), "sec#2")).toBe("sec#2");
  expect(refLabel(run([]), "4")).toBe("#4");
});

it("asks for suggested fixes in order, each with its findings' locations once", () => {
  const byId = new Map([
    ["1", finding("1")],
    [
      "2",
      finding("2", {
        locations: [
          { path: "src/a.rs", start: 2, end: 3 },
          { path: "b.rs", start: null, end: null },
        ],
      }),
    ],
  ]);
  const text = actionsText(
    "@",
    [
      { action: "Lock first", closes: ["1", "2"] },
      { action: "Add a test", closes: [] },
    ],
    byId,
  );
  expect(text).toBe(
    "Make these 2 fixes from a review of `@`. Change only what each one needs.\n\n" +
      "1. Lock first\n   `src/a.rs:2-3`, `b.rs`\n2. Add a test",
  );
});

it("tallies by severity, most severe first, skipping empty ones", () => {
  expect(tally([finding("1", { severity: "low" }), finding("2"), finding("3")])).toEqual([
    ["high", 2],
    ["low", 1],
  ]);
});

it("writes a finding as markdown for a comment", () => {
  const text = findingText(finding("1"), "sec");
  expect(text.startsWith("**🟠 HIGH — Title 1** (sec)")).toBe(true);
  expect(text).toContain("`src/a.rs:2-3`");
  expect(text).toContain("**Fix:** Do this.");
  expect(locationText({ path: "a", start: 4, end: 4 })).toBe("a:4");
});

describe("place", () => {
  // New lines: 1 a, 2 B, 3 c, 4 d. "x" was removed between a and B.
  const files = flatten(review([file("src/a.rs", [rows([" a", "-x", "+B", " c", " d"])])]));

  it("covers the changed rows in the range", () => {
    expect(place(files, { path: "src/a.rs", start: 1, end: 2 })).toEqual({
      file: 0,
      row: 0,
      changed: [1, 2],
    });
  });

  it("falls back to the nearest changed row when the range has none", () => {
    expect(place(files, { path: "a.rs", start: 4, end: 4 })).toEqual({
      file: 0,
      row: 4,
      changed: [2],
    });
  });

  it("finds nothing for files outside the review", () => {
    expect(place(files, { path: "src/b.rs", start: 1, end: 1 })).toBeUndefined();
  });
});
