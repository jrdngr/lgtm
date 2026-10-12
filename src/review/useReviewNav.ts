import type { DockviewApi, DockviewPanelApi } from "dockview-react";
import type { CommentDto, NoteDto } from "../bindings";
import { openFileIn } from "../editor/open";
import {
  commentJumpTarget,
  type DisplayLine,
  type FlatFile,
  fileJumpTarget,
  fileTargets,
  hunkTargets,
  type JumpTarget,
  rowJumpTarget,
  stepTarget,
} from "./model";
import { stepTour } from "./tour";

interface Nav {
  review: number | undefined;
  lines: DisplayLine[];
  files: FlatFile[];
  comments: CommentDto[];
  notes: NoteDto[];
  cursor: number;
  place(line: number): void;
  setScrollTo(to: { line: number }): void;
  api: DockviewPanelApi;
  containerApi: DockviewApi;
  focusRoot(): void;
}

/** Moving the review's cursor to files, rows, comments and notes, and scrolling there. */
export function useReviewNav(n: Nav) {
  const { lines, files } = n;

  function jumpTo(target: JumpTarget) {
    n.place(target.cursor);
    n.setScrollTo({ line: target.top });
    n.api.setActive();
    n.focusRoot();
  }

  function jump(file: number) {
    const target = fileJumpTarget(lines, file);
    if (target) jumpTo(target);
  }

  function jumpToComment(id: number) {
    const comment = n.comments.find((x) => x.id === id);
    if (!comment) return;
    const target = commentJumpTarget(lines, id);
    if (target) jumpTo(target);
    else jump(comment.file);
  }

  function jumpToRow(file: number, row: number) {
    const at = lines.findIndex((l) => l.kind === "rows" && l.file === file && l.rows.includes(row));
    if (at < 0) return jump(file);
    jumpTo(rowJumpTarget(at));
  }

  /** Goes to the next or previous note of the walkthrough's active tour. */
  function stepNote(dir: 1 | -1) {
    if (n.review === undefined) return;
    const note = stepTour(n.review, n.notes, dir);
    if (!note) return;
    if (note.rows) jumpToRow(note.file, note.rows.start);
    else jump(note.file);
  }

  function stepHunk(dir: 1 | -1) {
    const target = stepTarget(hunkTargets(lines), n.cursor, dir);
    if (target) jumpTo(target);
  }

  function stepFile(dir: 1 | -1) {
    const target = stepTarget(fileTargets(lines), n.cursor, dir);
    if (target) jumpTo(target);
  }

  /** Shows `line` of `path`: the row on that new-side line, or the nearest one. A file the
   * review doesn't change opens in the editor. */
  function showLine(path: string, line: number | null) {
    const file = files.findIndex((f) => f.dto.path === path || f.dto.oldPath === path);
    if (file < 0) return openFileIn(n.containerApi, path, line ?? undefined);
    if (line === null) return jump(file);
    const rows = files[file].rows;
    let best = -1;
    rows.forEach((r, i) => {
      if (r.newLine === null) return;
      if (best < 0 || Math.abs(r.newLine - line) < Math.abs((rows[best].newLine ?? 0) - line))
        best = i;
    });
    if (best < 0) jump(file);
    else jumpToRow(file, best);
  }

  return { jump, jumpToComment, jumpToRow, stepNote, stepHunk, stepFile, showLine };
}
