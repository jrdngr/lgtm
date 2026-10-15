import type { CommentDto } from "../bindings";
import type { DisplayLine } from "./model";

/** Blocks for comments on the PR and their quotes, then threads whose file the review doesn't
 * show, above every file. */
export const prLines = (comments: CommentDto[]): DisplayLine[] =>
  comments.map((c) => ({ kind: "comment", file: -1, hunk: -1, comment: c.id }));

/** The bar over the comments on the PR, or over the threads whose file the review doesn't
 * show, when `id` is the first of them. */
export function PrCommentsHead({ comments, id }: { comments: CommentDto[]; id: number }) {
  const onPr = comments.filter((c) => !c.path);
  const lost = comments.filter((c) => c.path);
  const head = (title: string, n: number) => (
    <div className="pr-comments-head">
      <b>{title}</b>
      <span className="faint">{n}</span>
    </div>
  );
  if (id === onPr[0]?.id) return head("PR comments", onPr.filter((c) => !c.thread?.quote).length);
  if (id === lost[0]?.id) return head("Files not in review", lost.length);
  return null;
}
