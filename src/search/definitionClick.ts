import { EditorView } from "@codemirror/view";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { definitions, diffSpot, editorSpot, openPlace, type SymbolSpot } from "./lsp";
import { goToDefinition } from "./store";
import { editorWordAt, identifierAt } from "./symbolAt";

/** The identifier a click is on, in an editor or a diff's code, and where it is for a
 * language server; no word when it's on none. */
function clicked(ws: WorkspaceName, e: MouseEvent): { word: string; spot?: SymbolSpot } {
  const t = e.target instanceof Element ? e.target : null;
  const editor = t?.closest(".cm-content")?.closest(".cm-editor");
  const view = editor instanceof HTMLElement ? EditorView.findFromDOM(editor) : null;
  if (view) {
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
    const word = editorWordAt(view, e.clientX, e.clientY);
    return { word, spot: pos === null ? undefined : editorSpot(view, pos) };
  }
  if (!t?.closest(".code")) return { word: "" };
  return { word: identifierAt(e.clientX, e.clientY), spot: diffSpot(ws, e.clientX, e.clientY) };
}

/** Opens the definition of the symbol at `spot`, named `word`: where the language server
 * says, else as lgtm's own search finds it. */
async function open(ws: WorkspaceName, word: string, spot?: SymbolSpot) {
  const places = spot && (await definitions(ws, spot).catch(() => undefined));
  if (places) openPlace(ws, places[0]);
  else goToDefinition(ws, word);
}

/** ⌘-click on a symbol in an editor or a diff opens its definition in `workspace()`. */
export function watchDefinitionClicks(workspace: () => WorkspaceName) {
  const down = (e: MouseEvent) => {
    if (e.button !== 0 || !(isMac ? e.metaKey : e.ctrlKey) || e.shiftKey || e.altKey) return;
    const ws = workspace();
    const { word, spot } = clicked(ws, e);
    if (!word) return;
    e.preventDefault();
    e.stopPropagation();
    open(ws, word, spot);
  };
  window.addEventListener("mousedown", down, true);
  return () => window.removeEventListener("mousedown", down, true);
}
