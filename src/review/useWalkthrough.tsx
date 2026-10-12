import { useMemo } from "react";
import type { NoteDto, ReviewDto } from "../bindings";
import type { FlatFile } from "./model";
import { flowLabel, NoteBlock, noteCommentText } from "./NoteBlock";
import { noteSlots } from "./notes";
import { rowsText } from "./sendText";
import { currentNote, useTourState } from "./tour";

const NO_NOTES: NoteDto[] = [];

/** The review's walkthrough notes as blocks under their code, following the walkthrough
 * panel's filters. */
export function useWalkthrough(
  review: ReviewDto | undefined,
  files: FlatFile[],
  commentOn: (file: number, rows: number[], body: string, snippet: string) => Promise<boolean>,
) {
  const tour = useTourState(review?.id);
  const notes = review?.state.notes ?? NO_NOTES;
  const slots = useMemo(
    () =>
      tour.inline ? noteSlots(notes.filter((n) => !tour.hidden.includes(n.category))) : undefined,
    [notes, tour.inline, tour.hidden],
  );
  const current = review ? currentNote(review.id, notes) : undefined;

  function comment(n: NoteDto) {
    const f = files[n.file];
    const span = n.rows;
    if (!f || !span) return Promise.resolve(false);
    const rows: number[] = [];
    for (let r = span.start; r <= span.end; r++) if (f.rows[r]?.kind !== "context") rows.push(r);
    return commentOn(n.file, rows, noteCommentText(n), rowsText(f, rows));
  }

  function renderNote(id: number) {
    const n = notes.find((x) => x.id === id);
    if (!n) return null;
    return (
      <NoteBlock
        note={n}
        current={current?.id === id}
        flowLabel={flowLabel(n, notes)}
        onComment={comment}
      />
    );
  }

  return { notes, slots, renderNote };
}
