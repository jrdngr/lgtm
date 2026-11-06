import { EditorState, type Extension, Prec, type Transaction } from "@codemirror/state";
import { type EditorView, keymap } from "@codemirror/view";
import { type MdEdit, mdEnter, mdRenumber, mdSmart, mdToggleCheck } from "./mdEdit";

/** Runs `f` at the cursor, when nothing is selected, as one change. */
function command(f: (text: string, at: number) => MdEdit | undefined) {
  return (view: EditorView) => {
    const sel = view.state.selection.main;
    if (!sel.empty) return false;
    const edit = f(view.state.doc.toString(), sel.head);
    if (!edit) return false;
    view.dispatch({
      changes: edit.changes,
      selection: { anchor: edit.cursor },
      scrollIntoView: true,
      userEvent: "input",
    });
    return true;
  };
}

/** After a delete, paste or drag in an ordered list, its numbers follow on again. */
const renumber = EditorState.transactionFilter.of((tr: Transaction) => {
  if (!tr.docChanged || !["delete", "input.paste", "move"].some((e) => tr.isUserEvent(e))) {
    return tr;
  }
  const edit = mdRenumber(tr.newDoc.toString(), tr.newSelection.main.head);
  return edit ? [tr, { changes: edit.changes, sequential: true }] : tr;
});

/** Markdown keys: Enter goes on with lists, quotes and tables, Shift-Tab is the smart key
 * for tables and footnotes, and ⌘↵ checks a checkbox. */
export function markdownKeys(): Extension {
  return [
    Prec.high(
      keymap.of([
        { key: "Enter", run: command(mdEnter) },
        { key: "Shift-Tab", run: command(mdSmart) },
        { key: "Mod-Enter", run: command(mdToggleCheck), preventDefault: true },
      ]),
    ),
    renumber,
  ];
}

/** Checks or unchecks the checkbox on line `line`, from 1, as a rendered view's click does.
 * Whether it changed. */
export function toggleCheckOn(view: EditorView, line: number): boolean {
  if (line < 1 || line > view.state.doc.lines) return false;
  const at = view.state.doc.line(line).from;
  const edit = mdToggleCheck(view.state.doc.toString(), at);
  if (!edit) return false;
  view.dispatch({ changes: edit.changes, userEvent: "input" });
  return true;
}
