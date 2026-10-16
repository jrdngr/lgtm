import type { CommentDto, StateDto } from "../bindings";
import { Inline } from "../bots/Markdown";
import { type CommandResult, commands, unwrap } from "../ipc";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { Icon } from "../ui/icons";
import { run } from "../ui/pathMenu";
import { confirmAction, promptText } from "../ui/prompt";
import { Empty } from "../ui/SectionHeader";
import { commentText } from "./comments";
import type { CurrentReview } from "./current";
import type { FlatFile } from "./model";

/** Whether an agent had the last word on the note. */
const replied = (c: CommentDto) => c.messages.at(-1)?.author === "agent";

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

interface ItemProps {
  c: CommentDto;
  review: CurrentReview;
  /** In a PR workspace, so a note can become a draft for GitHub. */
  canDraft: boolean;
}

function noteMenu({ c, review, canDraft }: ItemProps): CtxEntry[] {
  const apply = (r: Promise<CommandResult<StateDto>>) => run(unwrap(r).then(review.apply));
  async function edit() {
    const body = await promptText("Edit Note", { value: c.body, action: "Save", multiline: true });
    if (body !== undefined && body !== c.body) apply(commands.editComment(review.id, c.id, body));
  }
  async function remove() {
    if (await confirmAction("Delete this note?", "Delete")) {
      apply(commands.deleteComment(review.id, c.id));
    }
  }
  return [
    { label: "Jump to Note", onSelect: () => review.jumpToComment(c.id) },
    { label: "Edit", onSelect: edit },
    {
      label: "Done",
      checked: c.done,
      onSelect: () => apply(commands.setNoteDone(review.id, c.id, !c.done)),
    },
    { label: "Send to Terminal", onSelect: () => review.send(commentText(review.files, c)) },
    canDraft && {
      label: "Draft PR Comment",
      onSelect: () => apply(commands.draftFromNote(review.id, c.id)),
    },
    "-",
    { label: "Copy Text", onSelect: () => copy(c.body), disabled: !c.body },
    "-",
    { label: "Delete Note…", onSelect: remove },
  ];
}

function NoteItem(props: ItemProps) {
  const { c, review } = props;
  const line = lineOf(review.files, c);
  const agent = replied(c);
  return (
    <button
      type="button"
      className="comment-item"
      onClick={() => review.jumpToComment(c.id)}
      onContextMenu={contextMenu(() => noteMenu(props))}
    >
      <span className="item-icon">{agent && <Icon name="sparkle" />}</span>
      <span className="item-text">
        <Inline>{(c.body || "No text yet").split("\n")[0]}</Inline>
      </span>
      <span className="item-where">
        {agent && <span className="faint">Replied</span>}
        {line === undefined ? <span className="pill amber">Code changed</span> : line}
      </span>
    </button>
  );
}

interface Props {
  review: CurrentReview;
  notes: CommentDto[];
  canDraft: boolean;
  showDone: boolean;
  setShowDone(show: boolean): void;
}

/** The review's notes grouped by file, with done ones folded at the end. Click one to jump to
 * it. */
export function NoteList({ review, notes, canDraft, showDone, setShowDone }: Props) {
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
            <NoteItem key={c.id} c={c} review={review} canDraft={canDraft} />
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
          {showDone &&
            done.map((c) => <NoteItem key={c.id} c={c} review={review} canDraft={canDraft} />)}
        </div>
      )}
    </>
  );
}
