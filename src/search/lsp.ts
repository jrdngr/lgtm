import type { EditorView } from "@codemirror/view";
import type { Place } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { currentReview } from "../review/current";
import { workingCopy } from "../review/workingCopy";

/** Each open editor's file. */
const editorPaths = new WeakMap<EditorView, string>();

export function registerEditor(view: EditorView, path: string) {
  editorPaths.set(view, path);
}

/** A place in a file to ask a language server about: a 0-based line and UTF-16 column, with
 * the file's text when it isn't as on disk. */
export interface SymbolSpot {
  path: string;
  text: string | null;
  line: number;
  character: number;
}

/** The spot at `pos` in editor `v`, unless it's an editor lgtm doesn't know the file of. */
export function editorSpot(v: EditorView, pos: number): SymbolSpot | undefined {
  const path = editorPaths.get(v);
  if (!path) return undefined;
  const line = v.state.doc.lineAt(pos);
  return { path, text: v.state.sliceDoc(), line: line.number - 1, character: pos - line.from };
}

/** The spot at point `x`, `y` in a diff's code, when its row is a line of the working copy as
 * the review shows it. */
export function diffSpot(ws: WorkspaceName, x: number, y: number): SymbolSpot | undefined {
  const caret = document.caretRangeFromPoint?.(x, y);
  const cell = caret?.startContainer.parentElement?.closest<HTMLElement>("[data-path]");
  const code = cell?.querySelector(".code");
  const review = currentReview(ws);
  if (!caret || !cell || !code || review?.head !== workingCopy(ws)?.commitId) return undefined;
  const before = document.createRange();
  before.setStart(code, 0);
  before.setEnd(caret.startContainer, caret.startOffset);
  return {
    path: cell.dataset.path ?? "",
    text: null,
    line: Number(cell.dataset.line) - 1,
    character: before.toString().length,
  };
}

/** Where the symbol at `spot` is defined. Undefined when no language server answers, so the
 * caller can fall back on lgtm's own search. */
export async function definitions(
  ws: WorkspaceName,
  spot: SymbolSpot,
): Promise<Place[] | undefined> {
  const r = await commands.lspDefinition(ws, spot.path, spot.text, spot.line, spot.character);
  return r.status === "ok" && r.data?.length ? r.data : undefined;
}

/** Opens `place` in an editor with the cursor on it. */
export function openPlace(ws: WorkspaceName, place: Place) {
  openFile(ws, place.path, place.line + 1, undefined, {
    col: place.character,
    readOnly: place.external || undefined,
  });
}

/** The language server's markdown about the symbol at `spot`, if any. */
export async function hoverText(ws: WorkspaceName, spot: SymbolSpot): Promise<string | undefined> {
  const r = await commands.lspHover(ws, spot.path, spot.text, spot.line, spot.character);
  return r.status === "ok" ? (r.data ?? undefined) : undefined;
}
