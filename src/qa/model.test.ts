import { describe, expect, it } from "vitest";
import type { QState } from "../bindings";
import {
  applyText,
  current,
  dependents,
  lastAction,
  type Question,
  qqText,
  type Session,
  shownReply,
  step,
} from "./model";

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
    depends: [],
    state,
    answer: state === "decided" ? { option, note: null, other: null, at: 1 } : null,
    dropped: null,
    change: null,
    discussion: [],
  };
}

function session(over: Partial<Session> = {}): Session {
  return {
    id: "1",
    title: "T",
    prompt: "Add templates",
    attach: [],
    queue: { effects: [], deferrals: [], questions: [] },
    asked: false,
    note: "",
    plan: null,
    applied: null,
    sent: null,
    archived: false,
    created: 1,
    agent: null,
    nextComment: 0,
    ...over,
  };
}

describe("step", () => {
  it("goes from the prompt through questions to Apply", () => {
    expect(step(session())).toBe("prompt");
    expect(
      step(session({ queue: { effects: [], deferrals: [], questions: [question("Q1", "open")] } })),
    ).toBe("questions");
    const decided = [question("Q1", "decided", 1, 0), question("Q2", "dropped")];
    expect(step(session({ queue: { effects: [], deferrals: [], questions: decided } }))).toBe(
      "apply",
    );
    expect(step(session({ asked: true }))).toBe("apply");
  });
});

describe("current", () => {
  const d = session({
    queue: {
      effects: [],
      deferrals: [],
      questions: [
        question("Q1", "decided", 1, 0),
        question("Q2", "dropped"),
        question("Q3", "open"),
      ],
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

describe("applyText", () => {
  it("carries the decisions, the note, the plan and its comments", () => {
    const d = session({
      queue: {
        effects: [],
        deferrals: [],
        questions: [question("Q1", "decided", 1, 1), question("Q2", "open")],
      },
      note: " Keep it small ",
      plan: {
        files: [{ path: "a.rs", change: "edit", edits: ["Load", "Save"], decisions: ["Q1"] }],
        comments: [{ id: 1, quote: "Save", body: "Not yet" }],
      },
    });
    expect(applyText(d)).toBe(
      [
        "Add templates",
        "Decisions:\nQ1 — Subject Q1: Change it",
        "Also: Keep it small",
        "Plan:\n- a.rs (edit): Load; Save",
        'My comments on the plan:\n- On "Save": Not yet',
        "Write the changes these decisions call for into this repo.",
      ].join("\n\n"),
    );
  });
});

describe("shownReply", () => {
  it("hides the rewrite, even while its tag is still streaming in", () => {
    expect(shownReply('Fair.\n<rewrite>{"id":')).toBe("Fair.\n");
    expect(shownReply("Fair.\n<rew")).toBe("Fair.\n");
    expect(shownReply("Going with 2.\n<deci")).toBe("Going with 2.\n");
    expect(shownReply("a < b")).toBe("a < b");
  });
});

describe("dependents", () => {
  it("are the open and decided questions that list it, and its subquestions", () => {
    const listing = (q: Question, ...depends: string[]) => ({ ...q, depends });
    const questions = [
      question("Q1", "decided", 1, 0),
      question("Q1a", "open"),
      listing(question("Q2", "open"), "Q1"),
      listing(question("Q3", "decided", 1, 1), "Q1"),
      listing(question("Q4", "dropped"), "Q1"),
      listing(question("Q5", "deferred"), "Q1"),
      question("Q11", "open"),
    ];
    const d = session({ queue: { effects: [], deferrals: [], questions } });
    expect(dependents(d, "Q1").map((q) => q.id)).toEqual(["Q1a", "Q2", "Q3"]);
  });
});

describe("lastAction", () => {
  it("is whichever came last of a decision and a defer", () => {
    const questions = [question("Q1", "decided", 1, 0), question("Q2", "deferred")];
    const deferral = { id: "Q2", ids: ["Q2"], at: 5 };
    const d = (at: number) =>
      session({ queue: { effects: [], deferrals: [{ ...deferral, at }], questions } });
    expect(lastAction(d(5))).toEqual({ id: "Q2", defer: true });
    expect(lastAction(d(0.5))).toEqual({ id: "Q1", defer: false });
  });
});
