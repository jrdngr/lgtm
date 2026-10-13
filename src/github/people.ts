import type { CommentDto, PostDto } from "../bindings";

/** A stable hue for a name, so each person keeps one avatar color. */
export function hue(name: string): number {
  let h = 0;
  for (const ch of name) h = (h * 31 + (ch.codePointAt(0) ?? 0)) % 360;
  return h;
}

/** The viewer's GitHub login, known from a post in their pending review. */
export function viewerLogin(comments: CommentDto[]): string | undefined {
  for (const c of comments) {
    const mine = c.github?.posts.find((p) => p.pending);
    if (mine) return mine.author;
  }
  return undefined;
}

/** How a post's author shows: "You" for the viewer. */
export const authorName = (author: string, viewer: string | undefined) =>
  author === viewer ? "You" : author;

export type Verdict = "approved" | "changes" | "commented";

/** Each reviewer's standing verdict, in the order they first reviewed. An approval or a request
 * for changes stands until the next one, or until it's dismissed; a plain review only counts
 * for someone with no other verdict. */
export function verdicts(posts: PostDto[]): [string, Verdict][] {
  const out = new Map<string, Verdict>();
  for (const p of posts) {
    if (!p.verdict) continue;
    const had = out.get(p.author);
    if (p.verdict === "APPROVED") out.set(p.author, "approved");
    else if (p.verdict === "CHANGES_REQUESTED") out.set(p.author, "changes");
    else if (p.verdict === "DISMISSED" || !had) out.set(p.author, "commented");
  }
  return [...out];
}
