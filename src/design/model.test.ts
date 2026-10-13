import { describe, expect, it } from "vitest";
import type { QState } from "../bindings";
import { changedLines, current, type Design, type Question, qqText, step } from "./model";

function question(id: string, state: QState, round = 1, option: number | null = null): Question {
  return {
    id,
    round,
    text: {
      subject: `Subject ${id}`,
      context: ["A fact"],
      options: [
        { label: "Keep it", bullets: ["Cheap"] },
        { label: "Change it", bullets: [] },
      ],
      why: "It's simplest.",
    },
    sources: [],
    state,
    answer: state === "decided" ? { option, note: null, other: null, at: 1 } : null,
    dropped: null,
    change: null,
    discussion: [],
  };
}

function design(over: Partial<Design> = {}): Design {
  return {
    slug: "d",
    name: "D",
    dir: "docs/design/d",
    brief: "",
    hasDesign: true,
    queue: { questions: [], revised: 0 },
    queueError: null,
    docs: ["design.md", "brief.md"],
    diagrams: [],
    html: [],
    comments: [],
    attach: [],
    ...over,
  };
}

describe("step", () => {
  it("follows the loop from brief to refine", () => {
    expect(step(design({ hasDesign: false }))).toBe("brief");
    const open = { questions: [question("Q1", "open")], revised: 0 };
    expect(step(design({ queue: open }))).toBe("questions");
    const decided = { questions: [question("Q1", "decided", 1, 0)], revised: 0 };
    expect(step(design({ queue: decided }))).toBe("revise");
    const revised = { ...decided, revised: 1 };
    expect(step(design({ queue: revised }))).toBe("docs");
    expect(step(design({ queue: revised, diagrams: ["docs/flow.mmd"] }))).toBe("html");
    const html = [{ path: "html/design.html", source: "design.md", stale: false }];
    expect(step(design({ queue: revised, docs: ["design.md", "docs/a.md"], html }))).toBe("refine");
  });
});

describe("current", () => {
  const d = design({
    queue: {
      questions: [
        question("Q1", "decided", 1, 0),
        question("Q2", "dropped"),
        question("Q3", "open"),
      ],
      revised: 0,
    },
  });

  it("shows the question asked for, else the first open one", () => {
    expect(current(d, "Q1")?.id).toBe("Q1");
    expect(current(d, undefined)?.id).toBe("Q3");
    expect(current(d, "Q2")?.id).toBe("Q3");
  });
});

describe("qqText", () => {
  it("writes the qqueue template with the last decision first", () => {
    expect(qqText(question("Q2", "open"), question("Q1", "decided", 1, 1))).toBe(
      [
        "✅ Q1: Change it",
        "",
        "Q2 — Subject Q2",
        "",
        "Context:",
        "",
        "- A fact",
        "",
        "Options:",
        "",
        "1. Keep it (recommended)",
        "   - Cheap",
        "2. Change it",
        "",
        "Recommendation: option 1. It's simplest.",
      ].join("\n"),
    );
  });
});

describe("changedLines", () => {
  it("finds added and changed lines, and where lines went", () => {
    const { added, removed } = changedLines("a\nb\nc\nd", "a\nB\nc\nnew\nd");
    expect([...added]).toEqual([2, 4]);
    expect([...removed]).toEqual([2]);
  });

  it("marks lines gone from the end after the last line", () => {
    const { added, removed } = changedLines("a\nb\nc", "a");
    expect(added.size).toBe(0);
    expect([...removed]).toEqual([1]);
  });
});
