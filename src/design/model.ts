import type { DesignDto_Serialize, DesignQuestion_Serialize } from "../bindings";

export type Design = DesignDto_Serialize;
export type Question = DesignQuestion_Serialize;

/** The steps of a design, in loop order. */
export const STEPS = ["brief", "questions", "revise", "docs", "html", "refine"] as const;
export type Step = (typeof STEPS)[number];

export const STEP_NAMES: Record<Step, string> = {
  brief: "Brief",
  questions: "Questions",
  revise: "Revise",
  docs: "Docs",
  html: "HTML",
  refine: "Refine",
};

/** The Design tab's views. */
export type View = "brief" | "questions" | "docs" | "files";

/** The view where each step's work happens. */
export const STEP_VIEW: Record<Step, View> = {
  brief: "brief",
  questions: "questions",
  revise: "questions",
  docs: "files",
  html: "files",
  refine: "docs",
};

export const openQuestions = (d: Design) => d.queue.questions.filter((q) => q.state === "open");

/** Questions that count toward "N of M": open and decided, never dropped. */
export const countable = (d: Design) => d.queue.questions.filter((q) => q.state !== "dropped");

/** Decisions not yet folded into the design. */
export const unrevised = (d: Design) =>
  d.queue.questions.filter((q) => q.state === "decided" && q.round > d.queue.revised);

/** Where the design is in the loop: the first step with something left to do. */
export function step(d: Design): Step {
  if (!d.hasDesign) return "brief";
  if (openQuestions(d).length) return "questions";
  if (unrevised(d).length) return "revise";
  if (!d.docs.some((p) => p.startsWith("docs/")) && !d.diagrams.length) return "docs";
  if (!d.html.length) return "html";
  return "refine";
}

/** The question to show: `want` when it's still in the queue, else the first open one. */
export function current(d: Design, want: string | undefined): Question | undefined {
  const qs = d.queue.questions;
  const wanted = qs.find((q) => q.id === want && q.state !== "dropped");
  return wanted ?? qs.find((q) => q.state === "open");
}

/** The question decided most recently, for the ✅ line. */
export function lastDecided(d: Design): Question | undefined {
  let best: Question | undefined;
  for (const q of d.queue.questions) {
    if (q.state !== "decided") continue;
    if (!best || (q.answer?.at ?? 0) > (best.answer?.at ?? 0)) best = q;
  }
  return best;
}

/** An answer in a few words, for the rail and the ✅ line. */
export function answerLabel(q: Question): string {
  const a = q.answer;
  if (!a) return "";
  const picked =
    a.option === null
      ? `Other: ${a.other ?? ""}`
      : (q.text.options[a.option]?.label ?? `Option ${a.option + 1}`);
  return a.note ? `${picked} (${a.note})` : picked;
}

/** A question in the qqueue template, as plain text. */
export function qqText(q: Question, previous?: Question): string {
  const t = q.text;
  const lines: string[] = [];
  if (previous) lines.push(`✅ ${previous.id}: ${answerLabel(previous)}`, "");
  lines.push(`${q.id} — ${t.subject}`, "", "Context:", "");
  for (const c of t.context) lines.push(`- ${c}`);
  lines.push("", "Options:", "");
  t.options.forEach((o, i) => {
    lines.push(`${i + 1}. ${o.label}${i === 0 ? " (recommended)" : ""}`);
    for (const b of o.bullets) lines.push(`   - ${b}`);
  });
  lines.push("", `Recommendation: option 1. ${t.why}`);
  return lines.join("\n");
}

/** Every decision as a line of text. */
export const decisionsText = (d: Design) =>
  d.queue.questions
    .filter((q) => q.state === "decided")
    .map((q) => `${q.id} — ${q.text.subject}: ${answerLabel(q)}`)
    .join("\n");

/** Source lines (from 1) of `text` that are new or changed since `seen`, and the lines
 * after which `seen` had lines that are gone. A line diff by longest common subsequence. */
export function changedLines(
  seen: string,
  text: string,
): { added: Set<number>; removed: Set<number> } {
  const a = seen.split("\n");
  const b = text.split("\n");
  const n = a.length;
  const m = b.length;
  // Lengths of common subsequences of a[i..] and b[j..].
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
  const added = new Set<number>();
  const removed = new Set<number>();
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) {
      i++;
      j++;
    } else if (j < m && (i >= n || lcs[i][j + 1] >= lcs[i + 1][j])) {
      added.add(j + 1);
      j++;
    } else {
      removed.add(j);
      i++;
    }
  }
  return { added, removed };
}

/** A path's file name. */
export const fileName = (path: string) => path.split("/").at(-1) ?? path;
