import { EditorView } from "@codemirror/view";

const WORD = /[\p{L}\p{N}_$]/u;

/** The identifier around offset `at` in `text`, or "" off one. */
export function wordAt(text: string, at: number): string {
  let s = at;
  let e = at;
  while (s > 0 && WORD.test(text[s - 1])) s--;
  while (e < text.length && WORD.test(text[e])) e++;
  const word = text.slice(s, e);
  return /^[\p{L}_$]/u.test(word) ? word : "";
}

/** Selected text on one line, trimmed, or "". */
function oneLine(text: string): string {
  const t = text.trim();
  return t && !t.includes("\n") ? t : "";
}

/** The editor's selection on one line, else the identifier at its cursor. */
export function editorWord(v: EditorView): string {
  const { from, to, head } = v.state.selection.main;
  const picked = oneLine(v.state.sliceDoc(from, to));
  if (picked) return picked;
  const line = v.state.doc.lineAt(head);
  return wordAt(line.text, head - line.from);
}

/** The page's selection on one line, else the identifier at point `x`, `y`. */
export function wordAtPoint(x: number, y: number): string {
  return oneLine(window.getSelection()?.toString() ?? "") || identifierAt(x, y);
}

/** The identifier in the page's text at point `x`, `y`, or "". */
export function identifierAt(x: number, y: number): string {
  const range = document.caretRangeFromPoint?.(x, y);
  const node = range?.startContainer;
  if (!range || node?.nodeType !== Node.TEXT_NODE) return "";
  return wordAt(node.textContent ?? "", range.startOffset);
}

/** The word the focused editor or the page's selection is on, for keys like F12. */
export function focusedWord(): string {
  const el = document.activeElement?.closest?.(".cm-editor");
  const view = el instanceof HTMLElement ? EditorView.findFromDOM(el) : null;
  if (view) return editorWord(view);
  return oneLine(window.getSelection()?.toString() ?? "");
}

/** The identifier under point `x`, `y` in editor `v`, or "". */
export function editorWordAt(v: EditorView, x: number, y: number): string {
  const pos = v.posAtCoords({ x, y });
  if (pos === null) return "";
  const line = v.state.doc.lineAt(pos);
  return wordAt(line.text, pos - line.from);
}
