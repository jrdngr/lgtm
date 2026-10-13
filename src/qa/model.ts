import type { QaQuestion_Serialize, Session_Serialize } from "../bindings";

export type Session = Session_Serialize;
export type Question = QaQuestion_Serialize;

/** The steps of a session, in order. */
export const STEPS = ["prompt", "questions", "apply"] as const;
export type Step = (typeof STEPS)[number];

export const STEP_NAMES: Record<Step, string> = {
  prompt: "Prompt",
  questions: "Questions",
  apply: "Apply",
};

export const openQuestions = (d: Session) => d.queue.questions.filter((q) => q.state === "open");

/** Questions that count toward "N of M": open and decided, never dropped. */
export const countable = (d: Session) => d.queue.questions.filter((q) => q.state !== "dropped");

/** Where the session is: the first step with something left to do. */
export function step(d: Session): Step {
  if (!d.queue.questions.length) return "prompt";
  if (openQuestions(d).length) return "questions";
  return "apply";
}

/** The question to show: `want` when it's still in the queue, else the first open one. */
export function current(d: Session, want: string | undefined): Question | undefined {
  const qs = d.queue.questions;
  const wanted = qs.find((q) => q.id === want && q.state !== "dropped");
  return wanted ?? qs.find((q) => q.state === "open");
}

/** The question decided most recently, for the ✅ line. */
export function lastDecided(d: Session): Question | undefined {
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
export const decisionsText = (d: Session) =>
  d.queue.questions
    .filter((q) => q.state === "decided")
    .map((q) => `${q.id} — ${q.text.subject}: ${answerLabel(q)}`)
    .join("\n");

/** What a terminal agent needs to write the result: the prompt, the decisions, the note and
 * the plan. */
export function applyText(d: Session): string {
  const parts = [d.prompt.trim(), `Decisions:\n${decisionsText(d)}`];
  if (d.note.trim()) parts.push(`Also: ${d.note.trim()}`);
  if (d.plan) {
    const files = d.plan.files.map((f) => `- ${f.path} (${f.change}): ${f.edits.join("; ")}`);
    const comments = (d.plan.comments ?? []).map((c) => `- On "${c.quote}": ${c.body}`);
    parts.push(`Plan:\n${files.join("\n")}`);
    if (comments.length) parts.push(`My comments on the plan:\n${comments.join("\n")}`);
  }
  parts.push("Write the changes these decisions call for into this repo.");
  return parts.join("\n\n");
}

const TAG = "<rewrite>";

/** A streaming reply without the rewrite it may end with, which shows on the question
 * instead, including a tag still coming in. */
export function shownReply(text: string) {
  const at = text.indexOf(TAG);
  if (at >= 0) return text.slice(0, at);
  for (let n = TAG.length - 1; n > 0; n--)
    if (text.endsWith(TAG.slice(0, n))) return text.slice(0, -n);
  return text;
}
