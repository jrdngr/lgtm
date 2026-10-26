import { EditorView } from "@codemirror/view";
import { isMac } from "../core/keys";
import "./symbolLink.css";

const WORD = /[\p{L}\p{N}_$]/u;

/** The box of the identifier at `x`, `y` in an editor or a diff's code, if any. */
function wordBox(x: number, y: number): DOMRect | undefined {
  const t = document.elementFromPoint(x, y);
  const editor = t?.closest(".cm-content")?.closest(".cm-editor");
  const view = editor instanceof HTMLElement ? EditorView.findFromDOM(editor) : null;
  if (view) {
    const pos = view.posAtCoords({ x, y }, false);
    const word = pos === null ? null : view.state.wordAt(pos);
    const a = word && view.coordsAtPos(word.from);
    const b = word && view.coordsAtPos(word.to, -1);
    if (!a || !b || x < a.left || x > b.right) return undefined;
    return new DOMRect(a.left, a.top, b.right - a.left, a.bottom - a.top);
  }
  if (!t?.closest(".code")) return undefined;
  const caret = document.caretRangeFromPoint?.(x, y);
  const node = caret?.startContainer;
  if (!caret || !node || node.nodeType !== Node.TEXT_NODE) return undefined;
  const text = node.textContent ?? "";
  let s = caret.startOffset;
  let end = caret.startOffset;
  while (s > 0 && WORD.test(text[s - 1])) s--;
  while (end < text.length && WORD.test(text[end])) end++;
  if (s === end || !/^[\p{L}_$]/u.test(text[s])) return undefined;
  const r = document.createRange();
  r.setStart(node, s);
  r.setEnd(node, end);
  const box = r.getBoundingClientRect();
  return x < box.left || x > box.right ? undefined : box;
}

/** While ⌘ (Ctrl elsewhere) is held, underlines the symbol under the pointer in an editor or
 * a diff, as a link to its definition. */
export function watchSymbolLinks() {
  const line = document.createElement("div");
  line.className = "symbol-link";
  document.body.append(line);
  let x = -1;
  let y = -1;
  const show = (held: boolean) => {
    const box = held && x >= 0 ? wordBox(x, y) : undefined;
    document.documentElement.classList.toggle("symbol-link-on", !!box);
    line.hidden = !box;
    if (!box) return;
    const at = document.elementFromPoint(x, y);
    line.style.cssText = `left:${box.left}px;top:${box.bottom - 1}px;width:${box.width}px;background:${at ? getComputedStyle(at).color : ""}`;
  };
  const held = (e: KeyboardEvent | MouseEvent) => (isMac ? e.metaKey : e.ctrlKey);
  const move = (e: MouseEvent) => {
    x = e.clientX;
    y = e.clientY;
    show(held(e) && !e.buttons);
  };
  const key = (e: KeyboardEvent) => show(held(e));
  const off = () => show(false);
  window.addEventListener("mousemove", move, true);
  window.addEventListener("keydown", key, true);
  window.addEventListener("keyup", key, true);
  window.addEventListener("wheel", off, true);
  window.addEventListener("blur", off);
  return () => {
    off();
    line.remove();
    window.removeEventListener("mousemove", move, true);
    window.removeEventListener("keydown", key, true);
    window.removeEventListener("keyup", key, true);
    window.removeEventListener("wheel", off, true);
    window.removeEventListener("blur", off);
  };
}
