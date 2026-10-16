import type { CommentDto } from "../bindings";
import { codeText, conversationText } from "./comments";
import type { FlatFile } from "./model";
import { syncMarker } from "./syncedChat";

/** What starts a synced chat on a thread: the thread, the reply so far, and how the agent
 * hands back a reply. */
export function syncText(files: FlatFile[], c: CommentDto): string {
  const quote = c.thread?.quote;
  const posts = c.github?.posts.map((p) => `${p.author} wrote:\n${p.body.trim()}`) ?? [];
  return [
    `This chat is synced with ${syncMarker(c.id)}, a review thread on my PR. What we say here shows in the thread.`,
    quote ? `${quote.author} wrote, in a comment on the PR:\n${quote.text}` : "",
    c.path ? codeText(files, c) : "",
    ...(quote ? [] : posts),
    c.body.trim() ? `My reply so far:\n${c.body.trim()}` : "",
    conversationText(c.messages),
    `When I ask for a reply, call lgtm's suggest_reply tool with thread_id ${c.id}. It never posts to GitHub.`,
    "Start by telling me what you make of it.",
  ]
    .filter(Boolean)
    .join("\n\n");
}
