import type { CommentDto, ThreadDto } from "../bindings";

/** A comment on the PR itself, rather than on its code. */
export const onPr = (t: { path: string; github: unknown }) => !t.path && !!t.github;

/** The comment `t` quotes part of, if it's a quote. */
export const quoteOf = (t: ThreadDto) => t.state.quote?.of;

/** Comments on the PR, each followed by its quotes in the order they were made. */
export function prThreads(threads: ThreadDto[]): ThreadDto[] {
  const quotes = threads.filter((t) => quoteOf(t) !== undefined).sort((a, b) => a.id - b.id);
  return threads.filter(onPr).flatMap((t) => [t, ...quotes.filter((q) => quoteOf(q) === t.id)]);
}

/** Threads on code whose file the review doesn't show, like one deleted since. */
export function lostThreads(threads: ThreadDto[], comments: CommentDto[]): ThreadDto[] {
  const shown = new Set(comments.map((c) => c.id));
  return threads.filter((t) => t.path && !shown.has(t.id));
}

/** Comments on the PR and their quotes, then threads whose file the review doesn't show, as
 * comment blocks with no code to sit under. */
export function prComments(threads: ThreadDto[], comments: CommentDto[]): CommentDto[] {
  return [...prThreads(threads), ...lostThreads(threads, comments)].map((t) => ({
    id: t.id,
    path: t.path,
    file: -1,
    rows: null,
    body: t.body,
    snippet: t.snippet,
    review: "",
    createdAt: t.createdAt,
    updatedAt: t.createdAt,
    messages: t.messages,
    suggestion: t.suggestion,
    github: t.github,
    thread: t.state,
    done: false,
  }));
}

/** Quotes of comment `id`, oldest first. */
export const quotesOf = <T extends { id: number; thread?: ThreadDto["state"] | null }>(
  comments: T[],
  id: number,
) => comments.filter((c) => c.thread?.quote?.of === id).sort((a, b) => a.id - b.id);

/** Comments on the PR that have quotes, whose quotes stand in for them. */
export function quoted(threads: ThreadDto[]): Set<number> {
  return new Set(threads.flatMap((t) => (t.state.quote ? [t.state.quote.of] : [])));
}

/** A reply that opens with the text it answers, quoted. Matches what a quote's own Send
 * posts. */
export function quotedReply(quote: string, reply: string): string {
  return `> ${quote.trim().replaceAll("\n", "\n> ")}\n\n${reply.trim()}`;
}
