import type { MarkState, ReviewDto, StateDto } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { NOT_AT_WORKING_COPY } from "./CommentBlock";
import { draftFor } from "./comments";
import type { MenuAction } from "./MarkMenu";
import { changedRows, type DisplayLine, type FlatFile } from "./model";
import { selectedRowText, selectionText } from "./sendText";
import type { Comments } from "./useComments";
import { clearTextSelection } from "./useRowMouse";
import type { SelectionApi } from "./useSelection";

interface Deps {
  review: ReviewDto | undefined;
  files: FlatFile[];
  lines: DisplayLine[];
  s: SelectionApi;
  c: Comments;
  setError(message: string | undefined): void;
  /** Shows a new state after an edit. */
  edited(state: StateDto): void;
  /** Lets checked or unchecked files collapse or expand again. */
  followChecks(files: number[]): void;
  sendText(text: string, onSent?: () => void): void;
  closeMenu(): void;
}

/** What the review does to its selection: mark, check, comment, copy, and send. */
export function useReviewActions(d: Deps) {
  const { review, files, lines, s, setError } = d;
  const { sel } = s;

  function settle(r: CommandResult<StateDto>, onOk = d.edited) {
    if (r.status === "ok") onOk(r.data);
    else setError(r.error);
  }

  function applied(state: StateDto, checked: number[] = []) {
    d.edited(state);
    d.followChecks(checked);
  }

  /** Clears the selection to the cursor, as every action on a selection does. */
  function finishAction() {
    s.collapse();
    clearTextSelection();
  }

  async function applyMark(state: MarkState | null) {
    if (!review) return;
    const rows = changedRows(files, lines, sel.start, sel.end);
    if (rows.length === 0) return;
    const line = lines[sel.start];
    const check = sel.level === "file" && state === "reviewed" && line ? [line.file] : [];
    settle(await commands.markRows(review.id, rows, state, check), (next) => {
      finishAction();
      applied(next, check);
    });
  }

  async function setChecked(file: number, checked: boolean) {
    if (!review) return;
    settle(await commands.setFilesChecked(review.id, [file], checked), (next) =>
      applied(next, [file]),
    );
  }

  async function step(undo: boolean) {
    if (!review) return;
    settle(await (undo ? commands.undoMarks(review.id) : commands.redoMarks(review.id)));
  }

  async function resetMarks() {
    if (!review) return;
    settle(await commands.resetReviewMarks(review.id));
  }

  function copy() {
    const text = selectedRowText(files, lines, sel.start, sel.end);
    if (text !== undefined) navigator.clipboard.writeText(text);
  }

  function sendSelection() {
    const text = selectionText(files, lines, sel.start, sel.end);
    if (text) d.sendText(text, finishAction);
  }

  /** Opens a note on the selection, in its ask box when it's for a change. `github` makes it a
   * draft for the PR instead. */
  function startComment(field: "body" | "ask" = "body", github = false) {
    if (github && !d.c.canDraft) {
      setError("Drafts for GitHub need a PR workspace. Press c for a note.");
      return;
    }
    const draft = draftFor(files, lines, sel.start, sel.end);
    if (!draft) {
      setError("Select changed rows to comment on.");
      return;
    }
    setError(undefined);
    d.c.start({ ...draft, github }, field);
    finishAction();
  }

  function pick(action: MenuAction) {
    d.closeMenu();
    if (action === "copy") copy();
    else if (action === "agent") sendSelection();
    else if (action === "comment") startComment();
    else if (action === "change") {
      if (review?.hasWorkingCopy) startComment("ask");
      else setError(NOT_AT_WORKING_COPY);
    } else applyMark(action === "clear" ? null : (action as MarkState));
  }

  return { applyMark, setChecked, step, resetMarks, copy, startComment, pick };
}
