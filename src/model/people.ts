import type { CommentDto } from "../bindings";

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
