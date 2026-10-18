import type { CommentDto, PrDto, ThreadDto } from "../bindings";
import { threadWhere } from "../threads/stages";
import { codeText } from "./comments";
import type { FlatFile } from "./model";

/** A thread for a terminal agent: what's quoted or posted, its code and the reply so far. */
export function threadText(files: FlatFile[], c: CommentDto): string[] {
  const quote = c.thread?.quote;
  const posts = c.github?.posts.map((p) => `${p.author} wrote:\n${p.body.trim()}`) ?? [];
  return [
    quote ? `${quote.author} wrote, in a comment on the PR:\n${quote.text}` : "",
    c.path ? codeText(files, c) : "",
    ...(quote ? [] : posts),
    c.body.trim() ? `My reply so far:\n${c.body.trim()}` : "",
  ];
}

const join = (parts: string[]) => parts.filter(Boolean).join("\n\n");

/** How the brief a linked agent gets starts, so its prompt isn't filed in a thread's chat. */
export const LINK_START = "You're linked to lgtm's review of PR";

/** What links a terminal agent to the PR review: how it finds the threads, how it hears which
 * one I'm on, and how it hands back takes and replies. */
export function linkText(pr: PrDto, rules: string): string {
  return join([
    `${LINK_START} #${pr.number} "${pr.title}". lgtm's list_threads tool lists its threads.`,
    "Each prompt I send names the thread I'm on. Read the code it points at before you answer.",
    "Use set_thread_take for your verdict, suggest_reply for a reply in my voice, and set_thread_stage to move a thread. Nothing you do posts to GitHub.",
    `My rules for takes and replies:\n\n${rules.trim()}`,
    "Say you're ready and wait for me.",
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
  return { short, full: c ? join([short, ...threadText(files, c)]) : short };
}

/** What has a terminal agent review my reply on a thread against my rules for replies. */
export function reviewText(files: FlatFile[], c: CommentDto, rules: string): string {
  return join([
    "Review my reply to this review thread on my PR. Read the code it points at first.",
    ...threadText(files, c),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `Tell me in a few short bullets what you'd change and why. If you'd change the reply, call lgtm's suggest_reply tool with thread_id ${c.id} and the whole new reply. It never posts to GitHub.`,
    `Whenever your take on the thread changes, call lgtm's set_thread_take tool with thread_id ${c.id}, the verdict and why, and no reply.`,
  ]);
}

/** What has a terminal agent make a new thread's first take and reply. */
export function triageText(files: FlatFile[], c: CommentDto, rules: string): string {
  return join([
    "Triage this review thread on my PR. Read the code it points at first.",
    ...threadText(files, c),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `Then call lgtm's set_thread_take tool with thread_id ${c.id}, your verdict and why, and the reply, left out when I'd fix it exactly as asked. It never posts to GitHub. Tell me your take in a few short bullets.`,
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
    ...threads.map(([t, c]) =>
      join([`Thread ${t.id} (${threadWhere(t)}):`, ...threadText(files, c)]),
    ),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    "For each thread, call lgtm's set_thread_take tool with its thread_id, your verdict and why, and the reply, left out when I'd fix it exactly as asked. It never posts to GitHub. Then give me your takes in a few short bullets.",
  ]);
}
