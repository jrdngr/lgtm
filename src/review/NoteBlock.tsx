import { useState } from "react";
import type { NoteDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { IconButton } from "../ui/icons";
import { LevelPill, NoteIcon } from "./NoteIcon";
import { CATEGORIES } from "./notes";
import "./notes.css";

interface Props {
  note: NoteDto;
  /** The active tour is on this note. */
  current: boolean;
  /** Where it is in its flow, like "Login 2/5". */
  flowLabel?: string;
  /** What the note was kept as, when one of the review's comments is its text. */
  kept?: "note" | "draft";
  /** Keeps the note as one of yours, or with `draft`, as a draft for the PR. Resolves to
   * whether it worked. */
  onComment(note: NoteDto, draft: boolean): Promise<boolean>;
  canDraft: boolean;
}

/** A walkthrough note under its code. */
export function NoteBlock({ note, current, flowLabel, kept, onComment, canDraft }: Props) {
  /** A keep is being saved, so a second click can't add it twice. */
  const [saving, setSaving] = useState(false);
  const keep = async (draft: boolean) => {
    setSaving(true);
    await onComment(note, draft);
    setSaving(false);
  };
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
        {kept ? (
          <span className="faint">{kept === "note" ? "Kept as note" : "Drafted"}</span>
        ) : (
          <span className="note-actions reveal">
            <IconButton
              icon="comments"
              label="Keep as note"
              disabled={!note.rows || saving}
              onClick={() => keep(false)}
            />
            {canDraft && (
              <IconButton
                icon="pr"
                label="Draft PR comment"
                disabled={!note.rows || saving}
                onClick={() => keep(true)}
              />
            )}
          </span>
        )}
      </div>
      {note.body && (
        <div className="note-body">
          <Markdown>{note.body}</Markdown>
        </div>
      )}
    </div>
  );
}

/** The note as one of yours: its category and title, then its body. */
export const noteCommentText = (note: NoteDto) =>
  `${CATEGORIES[note.category].emoji} **${note.title}**\n\n${note.body}`.trim();
