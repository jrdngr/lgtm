import type { CommentDto, PrDto, Take, ThreadDto } from "../bindings";
import { threadWhere, VERDICTS } from "../model/stages";
import { codeText } from "./comments";
import type { FlatFile } from "./model";
import { UNTRUSTED_NOTE, untrusted } from "./untrusted";

/** A thread for a terminal agent: what's quoted or posted, its code, the take and the reply
 * so far.
 * GitHub's text is fenced, so a prompt holding it needs `UNTRUSTED_NOTE`. */
export function threadText(files: FlatFile[], c: CommentDto): string[] {
  const quote = c.thread?.quote;
  const take = c.thread?.take;
  const posts = c.github?.posts.map((p) => untrusted(p.body.trim(), p.author)) ?? [];
  return [
    quote ? `Quoted from a comment on the PR:\n${untrusted(quote.text, quote.author)}` : "",
    c.path ? codeText(files, c) : "",
    ...(quote ? [] : posts),
    take ? takeText(take) : "",
    c.body.trim() ? `My reply so far:\n${c.body.trim()}` : "",
  ];
}

const takeText = (t: Take) =>
  [`The take so far: ${VERDICTS[t.verdict][0]}. ${t.why.trim()}`, t.question?.trim()]
    .filter(Boolean)
    .join("\n");

const join = (parts: string[]) => parts.filter(Boolean).join("\n\n");

/** How a prompt names a thread to lgtm's thread tools. */
const ids = (c: CommentDto) => `thread_id ${c.id} and pr ${c.thread?.pr}`;

/** What has a terminal agent link its fix to a thread, which hands it back for review. */
export const fixedText = (c: CommentDto) =>
  `Once you've made the change this thread asks for, call lgtm's fixed_threads tool with pr ${c.thread?.pr} and thread_ids [${c.id}].`;

/** What lgtm's hook tells a linked terminal agent once: how it finds the PR's threads, how it
 * hears which one I'm on, and how it hands back takes and replies. */
export function linkText(pr: PrDto, rules: string): string {
  return join([
    `lgtm linked this terminal to its review of PR #${pr.number}. lgtm's list_threads tool lists its threads. Its title:\n${untrusted(pr.title, pr.author)}`,
    UNTRUSTED_NOTE,
    'lgtm adds the thread I\'m on to my prompts, so "this comment" or "the linked comment" means that thread. Read the code it points at before you answer.',
    `Use set_thread_take for your verdict, suggest_reply for a reply in my voice, and set_thread_stage to move a thread, each with pr ${pr.number}. Stop at a drafted reply: only I mark one ready. Nothing you do posts to GitHub.`,
    "Whenever what we discuss changes what a reply should say, call suggest_reply with the whole new reply right away, without asking. It only offers the reply; I choose whether to use it.",
    "When you change code a thread asks for, call fixed_threads right after that change, before the next one, so I can look each fix over on its own.",
    `My rules for takes and replies:\n\n${rules.trim()}`,
  ]);
}

/** Who started a thread and where it is, in a line. */
function named(t: ThreadDto): string {
  const who = t.state.quote?.author ?? t.github?.posts[0]?.author;
  const where = t.path ? `on ${t.path}${t.line != null ? `:${t.line}` : ""}` : "on the PR";
  return `lgtm: I'm on thread ${t.id}${who ? `, ${who}'s` : ""} ${where}.`;
}

/** The thread I'm on, as lgtm's hook adds it to my prompts: whole the first time, then by
 * name. */
export function threadContext(files: FlatFile[], t: ThreadDto, c: CommentDto | undefined) {
  const short = named(t);
  return { short, full: c ? join([short, UNTRUSTED_NOTE, ...threadText(files, c)]) : short };
}

/** What has a terminal agent review my reply on a thread against my rules for replies. */
export function reviewText(files: FlatFile[], c: CommentDto, rules: string): string {
  return join([
    "Review my reply to this review thread on my PR. Read the code it points at first.",
    UNTRUSTED_NOTE,
    ...threadText(files, c),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `Tell me in a few short bullets what you'd change and why. If you'd change the reply, call lgtm's suggest_reply tool with ${ids(c)} and the whole new reply. It never posts to GitHub.`,
    `Whenever your take on the thread changes, call lgtm's set_thread_take tool with ${ids(c)}, the verdict and why, and no reply.`,
  ]);
}

/** What has a terminal agent make a new thread's first take and reply. */
export function triageText(files: FlatFile[], c: CommentDto, rules: string): string {
  return join([
    "Triage this review thread on my PR. Read the code it points at first.",
    UNTRUSTED_NOTE,
    ...threadText(files, c),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `Then call lgtm's set_thread_take tool with ${ids(c)}, your verdict and why, a three to six word summary of what it asks, and the reply, left out when I'd fix it exactly as asked. It never posts to GitHub. Tell me your take in a few short bullets.`,
  ]);
}

/** What has a terminal agent triage every new thread at once. */
export function triageAllText(
  files: FlatFile[],
  threads: [ThreadDto, CommentDto][],
  rules: string,
): string {
  return join([
    "Triage these new review threads on my PR. Read the code each points at first.",
    UNTRUSTED_NOTE,
    ...threads.map(([t, c]) =>
      join([`Thread ${t.id} (${threadWhere(t)}):`, ...threadText(files, c)]),
    ),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `For each thread, call lgtm's set_thread_take tool with its thread_id and pr ${threads[0]?.[0].state.pr}, your verdict and why, a three to six word summary of what it asks, and the reply, left out when I'd fix it exactly as asked. It never posts to GitHub. Then give me your takes in a few short bullets.`,
  ]);
}
