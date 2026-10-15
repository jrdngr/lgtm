import type { GhPost, ThreadDto } from "../bindings";
import { threadLabel, threadWhere } from "./stages";

/** A thread's posts split at the viewer's last one: what came before, and what's unanswered
 * since. With no unanswered post, the last post is the one shown. */
export function unanswered(posts: GhPost[], viewer?: string): [GhPost[], GhPost[]] {
  let i = posts.length;
  while (i > 0 && posts[i - 1].author !== viewer) i--;
  const at = i === posts.length ? Math.max(posts.length - 1, 0) : i;
  return [posts.slice(0, at), posts.slice(at)];
}

/** A thread as text for a terminal agent: where it is, the code, and what was said. */
export function threadText(t: ThreadDto): string {
  const posts = (t.github?.posts ?? []).map((p) => `${p.author}: ${p.body.trim()}`);
  const code = t.snippet ? `\n${t.snippet}\n` : "";
  const quote = t.state.quote;
  const ask = quote
    ? `${quote.author}: ${quote.text.trim()}`
    : t.github
      ? posts.join("\n\n")
      : t.body.trim();
  return `${t.path || "The PR"} (${threadWhere(t)})${code}\n${ask}`;
}

/** The final replies laid out to paste elsewhere: final replies, threads fixed as asked, and
 * the ones still open. */
export function finalText(threads: ThreadDto[]): string {
  const head = (t: ThreadDto) => `${threadWhere(t)}: ${threadLabel(t)}`;
  const comment = (t: ThreadDto) => {
    const last = (t.state.quote?.text ?? t.github?.posts.at(-1)?.body)?.trim();
    return last ? `\n- Comment: "${last}"` : "";
  };
  const url = (t: ThreadDto) => (t.github?.url ? `\n- ${t.github.url}` : "");
  const ready = threads.filter((t) => t.state.stage === "ready");
  const replies = ready
    .filter((t) => t.body.trim())
    .map((t) => `${head(t)}${comment(t)}\n- Reply (final): "${t.body.trim()}"${url(t)}`);
  const fixed = ready.filter((t) => !t.body.trim()).map((t) => `${head(t)}${comment(t)}${url(t)}`);
  const open = threads
    .filter((t) => t.state.stage !== "ready" && t.state.stage !== "done")
    .map(head);
  return [
    ["Final replies", replies, "\n\n"],
    ["Fixed as asked", fixed, "\n\n"],
    ["Still open", open, "\n"],
  ]
    .filter(([, items]) => items.length > 0)
    .map(([title, items, sep]) => `${title}\n\n${(items as string[]).join(sep as string)}`)
    .join("\n\n");
}
