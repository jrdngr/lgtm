import type { Fixed, Stage, TakeVerdict, ThreadDto } from "../bindings";
import { quoted } from "./prComments";

/** Stages in work order, with their name and color. Decide sits first: untriaged threads
 * start there. */
export const STAGES: [Stage, string, string][] = [
  ["decide", "Decide", "amber"],
  ["toFix", "Fix", "blue"],
  ["review", "Review", "red"],
  ["reply", "Reply", "teal"],
  ["ready", "Ready", "green"],
  ["done", "Done", ""],
];

/** The CSS color of each tone `STAGES` names. */
const TONES: Record<string, string> = {
  amber: "var(--amber)",
  blue: "var(--accent)",
  red: "var(--flagged)",
  teal: "var(--github)",
  green: "var(--added)",
};

/** A stage's color, as its pill shows it. None for Done. */
export const stageColor = (s: Stage): string | undefined =>
  TONES[STAGES.find(([k]) => k === s)?.[2] ?? ""];

export const stageName = (s: Stage) => STAGES.find(([k]) => k === s)?.[1] ?? s;

export const VERDICTS: Record<TakeVerdict, [string, string]> = {
  agree: ["Agree", "green"],
  change: ["Change", "blue"],
  disagree: ["Disagree", "purple"],
  decide: ["Decide", "amber"],
  answer: ["Answer", ""],
};

/** Threads by stage, in `STAGES` order, keeping the threads' own order. Empty stages are left
 * out. */
export function byStage(threads: ThreadDto[]): [Stage, ThreadDto[]][] {
  return STAGES.map(([s]): [Stage, ThreadDto[]] => [
    s,
    threads.filter((t) => t.state.stage === s),
  ]).filter(([, ts]) => ts.length > 0);
}

/** Threads that take work: all but comments on the PR whose quotes stand in for them. */
export function working(threads: ThreadDto[]): ThreadDto[] {
  const q = quoted(threads);
  return threads.filter((t) => !q.has(t.id));
}

/** Threads waiting on the user: ones to triage or with a call to make, and fixes to look over. */
export const needsYou = (threads: ThreadDto[]) =>
  working(threads).filter((t) => t.state.stage === "decide" || t.state.stage === "review").length;

/** Threads no agent has made a take on yet. */
export const untriaged = (threads: ThreadDto[]) =>
  working(threads).filter((t) => t.state.stage === "decide" && !t.state.take);

/** A thread's one-line label: its summary, else what it quotes, else the start of its latest
 * post or draft. */
export function threadLabel(t: ThreadDto): string {
  if (t.state.summary) return t.state.summary;
  if (t.state.quote) return t.state.quote.text.split("\n")[0];
  const text = t.body || t.github?.posts.at(-1)?.body || "";
  // Bots open with hidden markers and headings.
  const line = text
    .replace(/<!--[\s\S]*?-->/g, "")
    .split("\n")
    .find((l) => l.trim());
  return line?.replace(/^#+\s*/, "") ?? "No text yet";
}

/** Where a thread is: `file.rs:42`, `file.rs`, or `PR` for a comment on the PR itself. */
export function threadWhere(t: ThreadDto): string {
  if (!t.path) return "PR";
  const name = t.path.split("/").pop();
  return t.line != null ? `${name}:${t.line}` : (name ?? t.path);
}

/** What a send does, counted from the ready threads. */
export interface SendPlan {
  replies: number;
  drafts: number;
  /** Ready threads with no reply: fixed as asked. */
  fixed: number;
  /** Fixed threads that get a "Done" reply. */
  doneReplies: number;
  /** Fixed review threads that get resolved. */
  resolves: number;
  /** Posts already in the viewer's pending review on GitHub. */
  pending: number;
}

export function sendPlan(threads: ThreadDto[], fixedAction: Fixed): SendPlan {
  const ready = threads.filter((t) => t.state.stage === "ready");
  const has = (t: ThreadDto) => !!t.body.trim();
  const fixed = ready.filter((t) => t.github && !has(t));
  const reply = fixedAction === "reply" || fixedAction === "replyResolve";
  const resolve = fixedAction === "resolve" || fixedAction === "replyResolve";
  return {
    replies: ready.filter((t) => t.github && has(t)).length,
    drafts: ready.filter((t) => !t.github && has(t)).length,
    fixed: fixed.length,
    doneReplies: reply ? fixed.length : 0,
    resolves: resolve ? fixed.filter((t) => t.path && !t.github?.resolved).length : 0,
    pending: threads.flatMap((t) => t.github?.posts ?? []).filter((p) => p.pending).length,
  };
}

export const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** What a send does, as a short list: "3 replies, 1 new comment, resolve 2". */
export function planText(p: SendPlan): string {
  const parts = [
    p.replies + p.doneReplies && count(p.replies + p.doneReplies, "reply", "replies"),
    p.drafts && count(p.drafts, "new comment", "new comments"),
    p.resolves && `resolve ${p.resolves}`,
    p.pending && count(p.pending, "pending post", "pending posts"),
  ].filter(Boolean);
  return parts.join(", ");
}

/** Whether a send has anything to send or mark done. */
export const canSend = (p: SendPlan) => p.replies + p.drafts + p.fixed + p.pending > 0;
