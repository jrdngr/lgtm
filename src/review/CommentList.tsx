import { useState } from "react";
import type { CommentDto } from "../bindings";
import { Inline } from "../bots/Markdown";
import { Avatar } from "../github/Post";
import { Empty } from "../ui/SectionHeader";
import type { CurrentReview } from "./current";
import type { FlatFile } from "./model";

type Filter = "open" | "pending" | "resolved" | "all";

const FILTERS: [Filter, string][] = [
  ["open", "Open"],
  ["pending", "Pending"],
  ["resolved", "Resolved"],
  ["all", "All"],
];

const pending = (c: CommentDto) => !!c.github?.posts.some((p) => p.pending);

const MATCH: Record<Filter, (c: CommentDto) => boolean> = {
  open: (c) => !c.github?.resolved,
  pending,
  resolved: (c) => !!c.github?.resolved,
  all: () => true,
};

/** Who spoke last, and what they said. */
function last(c: CommentDto, viewer: string | undefined): [string, string] {
  const post = c.github?.posts.at(-1);
  if (c.body || !post) return [viewer ?? "You", c.body || "No text yet"];
  return [post.author, post.body];
}

/** The line a comment ends on, in its file now. */
function lineOf(files: FlatFile[], c: CommentDto) {
  const row = c.rows ? files[c.file]?.rows[c.rows.end] : undefined;
  return row ? (row.newLine ?? row.oldLine) : undefined;
}

/** Comments in runs by file, keeping their order. */
function byFile(comments: CommentDto[]): [string, CommentDto[]][] {
  const out: [string, CommentDto[]][] = [];
  for (const c of comments) {
    const run = out.at(-1);
    if (run?.[0] === c.path) run[1].push(c);
    else out.push([c.path, [c]]);
  }
  return out;
}

interface Props {
  review: CurrentReview;
  viewer: string | undefined;
}

/** The review's comments grouped by file, with filters once some are GitHub threads. Click one
 * to jump to it. */
export function CommentList({ review, viewer }: Props) {
  const [filter, setFilter] = useState<Filter>("open");
  const { comments, files } = review;
  if (comments.length === 0) {
    return (
      <Empty>
        No comments. Select code and press <kbd>c</kbd>
      </Empty>
    );
  }
  const threads = comments.some((c) => c.github);
  const shown = threads ? comments.filter(MATCH[filter]) : comments;
  return (
    <>
      {threads && (
        <div className="filter-chips">
          {FILTERS.map(([f, label]) => {
            const n = comments.filter(MATCH[f]).length;
            return (
              <button
                key={f}
                type="button"
                className={`chip${filter === f ? " on" : ""}`}
                aria-pressed={filter === f}
                onClick={() => setFilter(f)}
              >
                {label}
                {f !== "all" && n > 0 && ` ${n}`}
              </button>
            );
          })}
        </div>
      )}
      {shown.length === 0 && <div className="list-empty">No {filter} comments</div>}
      {byFile(shown).map(([path, group]) => (
        <div key={path} className="comment-group">
          <div className="group-head" title={path}>
            <span className="group-path">{path}</span>
            <span className="spacer" />
            <span>{group.length}</span>
          </div>
          {group.map((c) => {
            const [who, text] = last(c, viewer);
            const line = lineOf(files, c);
            return (
              <button
                key={c.id}
                type="button"
                className="comment-item"
                onClick={() => review.jumpToComment(c.id)}
              >
                <Avatar name={who} />
                <span className="item-text">
                  <Inline>{text.split("\n")[0]}</Inline>
                </span>
                <span className="item-where">
                  {c.github?.resolved && filter === "all" && (
                    <span className="pill green">Resolved</span>
                  )}
                  {pending(c) && <span className="pill amber">Pending</span>}
                  {line === undefined ? <span className="pill amber">Code changed</span> : line}
                </span>
              </button>
            );
          })}
        </div>
      ))}
    </>
  );
}
