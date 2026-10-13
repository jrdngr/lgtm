import { useState } from "react";
import type { NoteDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { LevelPill, NoteIcon } from "./NoteIcon";
import { CATEGORIES } from "./notes";
import "./notes.css";

interface Props {
  note: NoteDto;
  /** The active tour is on this note. */
  current: boolean;
  /** Where it is in its flow, like "Login 2/5". */
  flowLabel?: string;
  /** Turns the note into a comment. Resolves to whether it worked. */
  onComment(note: NoteDto): Promise<boolean>;
}

/** A walkthrough note under its code. */
export function NoteBlock({ note, current, flowLabel, onComment }: Props) {
  const [commented, setCommented] = useState(false);
  const { label } = CATEGORIES[note.category];
  return (
    <div
      className={`note-block note-${note.category}${note.level ? ` level-${note.level}` : ""}${current ? " current" : ""}`}
    >
      <div className="note-head">
        <span title={note.level ? `${label}, ${note.level}` : label}>
          <NoteIcon category={note.category} level={note.level} />
        </span>
        <span className="note-title">{note.title}</span>
        {note.category === "important" && note.level && <LevelPill level={note.level} />}
        {flowLabel && <span className="pill">{flowLabel}</span>}
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
