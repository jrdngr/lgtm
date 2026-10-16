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

/** Whether the agent answered `q` itself. */
export const isAuto = (q: Question) => q.state === "decided" && !!q.answer?.auto;

export const autoAnswered = (d: Session) => d.queue.questions.filter(isAuto);

/** Questions that can be browsed: every one but the dropped. */
export const browsable = (d: Session) => d.queue.questions.filter((q) => q.state !== "dropped");

/** Questions that count toward "N of M": the browsable ones left to the user. */
export const countable = (d: Session) => browsable(d).filter((q) => !isAuto(q));

/** Where the session is: the first step with something left to do. */
export function step(d: Session): Step {
  if (!d.asked && !d.queue.questions.length) return "prompt";
  if (openQuestions(d).length) return "questions";
  return "apply";
}

/** Whether `q` is a subquestion of `parent`, like Q4a of Q4. */
export const isSubOf = (q: string, parent: string) =>
  q.startsWith(parent) && /^[a-z]$/.test(q.slice(parent.length));

/** The open and decided questions an answer to `id` can change: the ones that list it, and
 * its subquestions. */
export const dependents = (d: Session, id: string) =>
  d.queue.questions.filter(
    (q) =>
      q.id !== id &&
      (q.state === "open" || q.state === "decided") &&
      (q.depends.includes(id) || isSubOf(q.id, id)),
  );

/** The question to answer next: the first open one. Later ones can only be looked at. */
export const head = (d: Session) => d.queue.questions.find((q) => q.state === "open");

/** The question to show: `want` when it's still in the queue, else the first open one. */
export function current(d: Session, want: string | undefined): Question | undefined {
  const qs = d.queue.questions;
  const wanted = qs.find((q) => q.id === want && q.state !== "dropped");
  return wanted ?? qs.find((q) => q.state === "open");
}

/** The question the user decided most recently, for undo. */
export function lastDecided(d: Session): Question | undefined {
  let best: Question | undefined;
  for (const q of d.queue.questions) {
    if (q.state !== "decided" || isAuto(q)) continue;
    if (!best || (q.answer?.at ?? 0) > (best.answer?.at ?? 0)) best = q;
  }
  return best;
}

export type UndoKind = "decision" | "defer" | "reopen";

/** What undo takes back: the latest decision, defer or reopened auto answer. */
export function lastAction(d: Session): { id: string; kind: UndoKind } | undefined {
  const decided = lastDecided(d);
  const deferral = d.queue.deferrals.at(-1);
  const reopen = d.queue.reopens.at(-1);
  const all = [
    decided && { id: decided.id, kind: "decision" as UndoKind, at: decided.answer?.at ?? 0 },
    deferral && { id: deferral.id, kind: "defer" as UndoKind, at: deferral.at ?? 0 },
    reopen && { id: reopen.id, kind: "reopen" as UndoKind, at: reopen.at ?? 0 },
  ];
  let best: { id: string; kind: UndoKind; at: number } | undefined;
  for (const x of all) if (x && (!best || x.at > best.at)) best = x;
  return best && { id: best.id, kind: best.kind };
}

/** An answer in a few words. */
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
    if (o.means) lines.push(`   ${o.means}`);
    for (const b of o.pros) lines.push(`   - Pro: ${b}`);
    for (const b of o.cons) lines.push(`   - Con: ${b}`);
    for (const b of o.risks) lines.push(`   - Risk: ${b}`);
    for (const b of o.bullets ?? []) lines.push(`   - ${b}`);
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

/** What a terminal agent needs to write the result: the prompt, the decisions and the
 * note. */
export function applyText(d: Session): string {
  // A terminal agent asked these, and has the rest of the conversation.
  if (d.fromTerminal) return answersText(d);
  const parts = [d.prompt.trim(), `Decisions:\n${decisionsText(d)}`];
  const deferred = d.queue.questions.filter((q) => q.state === "deferred");
  if (deferred.length)
    parts.push(
      `Deferred, leave unimplemented: ${deferred.map((q) => `${q.id} (${q.text.subject})`).join(", ")}`,
    );
  if (d.note.trim()) parts.push(`Also: ${d.note.trim()}`);
  parts.push("Write the changes these decisions call for into this repo.");
  return parts.join("\n\n");
}

/** The decisions on questions a terminal agent asked, for it to carry on with. */
function answersText(d: Session): string {
  const parts = [`My answers to "${d.title}":\n${decisionsText(d)}`];
  const deferred = d.queue.questions.filter((q) => q.state === "deferred");
  if (deferred.length)
    parts.push(`Deferred: ${deferred.map((q) => `${q.id} (${q.text.subject})`).join(", ")}`);
  if (d.note.trim()) parts.push(`Also: ${d.note.trim()}`);
  return parts.join("\n\n");
}

const TAGS = ["<rewrite>", "<decide>"];

/** A streaming reply without the tags it may end with, which act on the question instead,
 * including a tag still coming in. */
export function shownReply(text: string) {
  const at = Math.min(...TAGS.map((t) => text.indexOf(t)).filter((i) => i >= 0));
  if (Number.isFinite(at)) return text.slice(0, at);
  for (let n = TAGS[0].length - 1; n > 0; n--)
    if (TAGS.some((t) => text.endsWith(t.slice(0, n)))) return text.slice(0, -n);
  return text;
}
