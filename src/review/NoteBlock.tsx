import { useState } from "react";
import type { NoteDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { CATEGORIES } from "./notes";
import "./notes.css";

interface Props {
  note: NoteDto;
  /** The active tour is on this note. */
  current: boolean;
  /** Where it is in its flow, like "Login, step 2 of 5". */
  flowLabel?: string;
  /** Turns the note into a comment. Resolves to whether it worked. */
  onComment(note: NoteDto): Promise<boolean>;
}

/** A walkthrough note under its code. */
export function NoteBlock({ note, current, flowLabel, onComment }: Props) {
  const [commented, setCommented] = useState(false);
  const { emoji, label } = CATEGORIES[note.category];
  return (
    <div
      className={`note-block note-${note.category}${note.level ? ` level-${note.level}` : ""}${current ? " current" : ""}`}
    >
      <div className="note-head">
        <span className="note-emoji" title={note.level ? `${label}, ${note.level}` : label}>
          {emoji}
        </span>
        <span className="note-title">{note.title}</span>
        {flowLabel && <span className="note-flow">{flowLabel}</span>}
        <span className="spacer" />
        <button
          type="button"
          className="quiet"
          disabled={commented || !note.rows}
          onClick={async () => setCommented(await onComment(note))}
          title="Add it as your comment"
        >
          {commented ? "Commented" : "Comment"}
        </button>
      </div>
      {note.body && (
        <div className="note-body">
          <Markdown>{note.body}</Markdown>
        </div>
      )}
    </div>
  );
}

/** The note as a comment: its category and title, then its body. */
export const noteCommentText = (note: NoteDto) =>
  `${CATEGORIES[note.category].emoji} **${note.title}**\n\n${note.body}`.trim();

/** Where a note is in its flow. */
export function flowLabel(note: NoteDto, notes: NoteDto[]): string | undefined {
  if (!note.flow) return undefined;
  const name = note.flow.flow;
  const steps = notes.filter((n) => n.flow?.flow === name).length;
  return `${name}, step ${note.flow.step} of ${steps}`;
}
