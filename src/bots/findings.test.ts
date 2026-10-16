import { describe, expect, it } from "vitest";
import type { BotRun, Finding, Part } from "../bindings";
import { file, review, rows } from "../review/fixture.test-utils";
import { flatten } from "../review/model";
import {
  actionsText,
  findingText,
  locationText,
  place,
  refLabel,
  report,
  tally,
  verdictOf,
} from "./findings";

const finding = (id: string, over: Partial<Finding> = {}): Finding => ({
  id,
  severity: "high",
  title: `Title ${id}`,
  locations: [{ path: "src/a.rs", start: 2, end: 3 }],
  facts: ["a fact"],
  explanation: "Why it matters.",
  fix: "Do this.",
  ...over,
});

const section = (persona: string, findings: Finding[]): Part => ({
  persona,
  output: { kind: "review", verdict: `${persona} verdict`, findings },
});

const fixes = (task: string): Part => ({
  persona: null,
  output: { kind: "minimal", task, fixes: [], offTask: [] },
});

const run = (parts: Part[]): BotRun => ({
  id: 1,
  workspace: null,
  revset: "@",
  from: null,
  to: "c",
  personas: [],
  parts,
  createdAt: 0,
});

describe("report", () => {
  it("gathers each reviewer's findings and the suggested fixes", () => {
    const r = report(run([section("sec", [finding("sec#1")]), fixes("t")]));
    expect([...r.byPersona.keys()]).toEqual(["sec"]);
    expect(r.minimal?.task).toBe("t");
    expect(verdictOf(r)).toBe("sec verdict");
  });

  it("has no overall verdict with several reviewers", () => {
    expect(verdictOf(report(run([section("a", []), section("b", [])])))).toBeUndefined();
  });
});

it("numbers a lone reviewer's findings without its name", () => {
  const lone = { ...run([]), personas: [{ name: "sec", description: "" }] };
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
