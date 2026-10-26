import { EditorView } from "@codemirror/view";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { goToDefinition } from "./store";
import { editorWordAt, identifierAt } from "./symbolAt";

/** The identifier a click is on, in an editor or a diff's code, or "". */
function clickedWord(e: MouseEvent): string {
  const t = e.target instanceof Element ? e.target : null;
  const editor = t?.closest(".cm-content")?.closest(".cm-editor");
  const view = editor instanceof HTMLElement ? EditorView.findFromDOM(editor) : null;
  if (view) return editorWordAt(view, e.clientX, e.clientY);
  return t?.closest(".code") ? identifierAt(e.clientX, e.clientY) : "";
}

/** ⌘-click on a symbol in an editor or a diff opens its definition in `workspace()`. */
export function watchDefinitionClicks(workspace: () => WorkspaceName) {
  const down = (e: MouseEvent) => {
    if (e.button !== 0 || !(isMac ? e.metaKey : e.ctrlKey) || e.shiftKey || e.altKey) return;
    const word = clickedWord(e);
    if (!word) return;
    e.preventDefault();
    e.stopPropagation();
    goToDefinition(workspace(), word);
  };
  window.addEventListener("mousedown", down, true);
  return () => window.removeEventListener("mousedown", down, true);
}
