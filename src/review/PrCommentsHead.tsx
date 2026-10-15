import type { CommentDto } from "../bindings";
import type { DisplayLine } from "./model";

/** Blocks for comments on the PR and their quotes, above every file. */
export const prLines = (comments: CommentDto[]): DisplayLine[] =>
  comments.map((c) => ({ kind: "comment", file: -1, hunk: -1, comment: c.id }));

/** The bar over the comments on the PR. */
export function PrCommentsHead({ comments }: { comments: CommentDto[] }) {
  return (
    <div className="pr-comments-head">
      <b>PR comments</b>
      <span className="faint">{comments.filter((c) => !c.thread?.quote).length}</span>
    </div>
  );
}
