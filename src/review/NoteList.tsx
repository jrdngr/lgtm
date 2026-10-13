import { useState } from "react";
import type { CommentDto } from "../bindings";
import { Inline } from "../bots/Markdown";
import { Icon } from "../ui/icons";
import { Empty } from "../ui/SectionHeader";
import type { CurrentReview } from "./current";
import type { FlatFile } from "./model";

/** Where the note's agent conversation stands: it changed the code, or it answered. */
function agentState(c: CommentDto): "changed" | "replied" | undefined {
  const last = c.messages.at(-1);
  if (last?.author !== "agent") return undefined;
  const asked = c.messages.filter((m) => m.author === "user").at(-1);
  return asked?.change ? "changed" : "replied";
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

function NoteItem({ c, review }: { c: CommentDto; review: CurrentReview }) {
  const line = lineOf(review.files, c);
  const agent = agentState(c);
  return (
    <button type="button" className="comment-item" onClick={() => review.jumpToComment(c.id)}>
      <span className="item-icon">
        {agent && <Icon name={agent === "changed" ? "edit" : "sparkle"} />}
      </span>
      <span className="item-text">
        <Inline>{(c.body || "No text yet").split("\n")[0]}</Inline>
      </span>
      <span className="item-where">
        {agent && <span className="faint">{agent === "changed" ? "Changed" : "Replied"}</span>}
        {line === undefined ? <span className="pill amber">Code changed</span> : line}
      </span>
    </button>
  );
}

/** The review's notes grouped by file, with done ones folded at the end. Click one to jump to
 * it. */
export function NoteList({ review, notes }: { review: CurrentReview; notes: CommentDto[] }) {
  const [showDone, setShowDone] = useState(false);
  if (notes.length === 0) {
    return (
      <Empty>
        No notes. Select code and press <kbd>c</kbd>
      </Empty>
    );
  }
  const open = notes.filter((c) => !c.done);
  const done = notes.filter((c) => c.done);
  return (
    <>
      {byFile(open).map(([path, group]) => (
        <div key={path} className="comment-group">
          <div className="group-head" title={path}>
            <span className="group-path">{path}</span>
            <span className="spacer" />
            <span>{group.length}</span>
          </div>
          {group.map((c) => (
            <NoteItem key={c.id} c={c} review={review} />
          ))}
        </div>
      ))}
      {done.length > 0 && (
        <div className="comment-group">
          <button
            type="button"
            className="group-head fold"
            aria-expanded={showDone}
            onClick={() => setShowDone(!showDone)}
          >
            <span className="chevron">{showDone ? "▾" : "▸"}</span>
            <span>Done</span>
            <span className="spacer" />
            <span>{done.length}</span>
          </button>
          {showDone && done.map((c) => <NoteItem key={c.id} c={c} review={review} />)}
        </div>
      )}
    </>
  );
}
