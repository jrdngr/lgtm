import { EditorView } from "@codemirror/view";
import { useSyncExternalStore } from "react";
import type { WorkspaceName } from "../core/workspace";
import { diffSpot, editorSpot, hoverText, type SymbolSpot } from "./lsp";

/** What the hover card shows, under the word at `x`, `top`, or above it at `bottom`. */
export interface Hover {
  text: string;
  x: number;
  top: number;
  bottom: number;
}

let shown: Hover | undefined;
const listeners = new Set<() => void>();
const set = (h: Hover | undefined) => {
  if (h === shown) return;
  shown = h;
  for (const l of listeners) l();
};

export function useHover(): Hover | undefined {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => shown,
  );
}

const hideHover = () => set(undefined);

const WAIT_MS = 350;
const WORD = /[\p{L}\p{N}_$]/u;

/** The word under the pointer in an editor or a diff's code: its box and where it is. */
function wordAt(
  ws: WorkspaceName,
  e: MouseEvent,
): { key: string; box: DOMRect; spot: SymbolSpot } | undefined {
  const t = e.target instanceof Element ? e.target : null;
  const editor = t?.closest(".cm-content")?.closest(".cm-editor");
  const view = editor instanceof HTMLElement ? EditorView.findFromDOM(editor) : null;
  if (view) {
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY }, false);
    if (pos === null) return undefined;
    const word = view.state.wordAt(pos);
    const spot = word && editorSpot(view, word.from);
    const a = word && view.coordsAtPos(word.from);
    const b = word && view.coordsAtPos(word.to, -1);
    if (!word || !spot || !a || !b || e.clientX < a.left || e.clientX > b.right) return undefined;
    const box = new DOMRect(a.left, a.top, b.right - a.left, a.bottom - a.top);
    return { key: `${spot.path}:${word.from}`, box, spot };
  }
  if (!t?.closest(".code")) return undefined;
  const caret = document.caretRangeFromPoint?.(e.clientX, e.clientY);
  const node = caret?.startContainer;
  const spot = diffSpot(ws, e.clientX, e.clientY);
  if (!caret || !node || node.nodeType !== Node.TEXT_NODE || !spot) return undefined;
  const text = node.textContent ?? "";
  let s = caret.startOffset;
  let end = caret.startOffset;
  while (s > 0 && WORD.test(text[s - 1])) s--;
  while (end < text.length && WORD.test(text[end])) end++;
  if (s === end) return undefined;
  const r = document.createRange();
  r.setStart(node, s);
  r.setEnd(node, end);
  const box = r.getBoundingClientRect();
  if (e.clientX < box.left || e.clientX > box.right) return undefined;
  return {
    key: `${spot.path}:${spot.line}:${spot.character - (caret.startOffset - s)}`,
    box,
    spot,
  };
}

/** Shows the language server's card for the symbol the pointer rests on, in an editor or a
 * diff of `workspace()`. */
export function watchHovers(workspace: () => WorkspaceName) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let over = "";
  const move = (e: MouseEvent) => {
    if (e.target instanceof Element && e.target.closest(".hover-card")) return;
    const ws = workspace();
    const at = e.buttons ? undefined : wordAt(ws, e);
    const key = at?.key ?? "";
    if (key === over) return;
    over = key;
    clearTimeout(timer);
    hideHover();
    if (!at) return;
    timer = setTimeout(async () => {
      const text = await hoverText(ws, at.spot).catch(() => undefined);
      if (text && over === key)
        set({ text, x: at.box.left, top: at.box.bottom, bottom: at.box.top });
    }, WAIT_MS);
  };
  const leave = () => {
    over = "";
    clearTimeout(timer);
    hideHover();
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mousedown", leave, true);
  window.addEventListener("keydown", leave, true);
  window.addEventListener("wheel", leave, true);
  return () => {
    clearTimeout(timer);
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mousedown", leave, true);
    window.removeEventListener("keydown", leave, true);
    window.removeEventListener("wheel", leave, true);
  };
}
