import type { CommentDto } from "../bindings";
import { codeText, conversationText } from "./comments";
import type { FlatFile } from "./model";
import { syncMarker } from "./syncedChat";

/** A thread for a terminal agent: what's quoted or posted, its code, the reply so far and
 * the chat. */
export function threadText(files: FlatFile[], c: CommentDto): string[] {
  const quote = c.thread?.quote;
  const posts = c.github?.posts.map((p) => `${p.author} wrote:\n${p.body.trim()}`) ?? [];
  return [
    quote ? `${quote.author} wrote, in a comment on the PR:\n${quote.text}` : "",
    c.path ? codeText(files, c) : "",
    ...(quote ? [] : posts),
    c.body.trim() ? `My reply so far:\n${c.body.trim()}` : "",
    conversationText(c.messages),
  ];
}

/** How the agent keeps the thread's take current. */
const takeText = (id: number) =>
  `Whenever your take on the thread changes, call lgtm's set_thread_take tool with thread_id ${id}, the verdict and why, and no reply.`;

/** What starts a synced chat on a thread: the thread, the reply so far, and how the agent
 * hands back a reply. */
export function syncText(files: FlatFile[], c: CommentDto): string {
  const take = c.thread?.take;
  return [
    `This chat is synced with ${syncMarker(c.id)}, a review thread on my PR. What we say here shows in the thread.`,
    ...threadText(files, c),
    `When I ask for a reply, call lgtm's suggest_reply tool with thread_id ${c.id}. It never posts to GitHub.`,
    takeText(c.id),
    take ? `Your take from triage (${take.verdict}):\n${take.why}` : "",
    take ? "Just say you're ready and wait for me." : "Start by telling me what you make of it.",
  ]
    .filter(Boolean)
    .join("\n\n");
}

/** What has a terminal agent review my reply on a thread against my rules for replies. */
export function reviewText(files: FlatFile[], c: CommentDto, rules: string): string {
  return [
    "Review my reply to this review thread on my PR. Read the code it points at first.",
    ...threadText(files, c),
    `My rules for takes and replies:\n\n${rules.trim()}`,
    `Tell me in a few short bullets what you'd change and why. If you'd change the reply, call lgtm's suggest_reply tool with thread_id ${c.id} and the whole new reply. It never posts to GitHub.`,
    takeText(c.id),
  ]
    .filter(Boolean)
    .join("\n\n");
}
