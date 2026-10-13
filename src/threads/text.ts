import type { GhPost, ThreadDto } from "../bindings";
import { threadWhere } from "./stages";

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
  const ask = t.github ? posts.join("\n\n") : t.body.trim();
  return `${t.path || "The PR"} (${threadWhere(t)})${code}\n${ask}`;
}
