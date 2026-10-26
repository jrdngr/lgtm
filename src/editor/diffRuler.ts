import { SearchCursor } from "@codemirror/search";
import type { EditorState } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { shownBlocks } from "./diffGutter";

const WIDTH = 10;

/** The color of a block's ticks, from the app's CSS variables. */
const COLOR = { added: "--added", changed: "--amber", removed: "--error" } as const;

/** Where the selected text shows up in the doc, as the selection's match highlight finds it:
 * one non-empty range within a line. */
function selectionMatches(state: EditorState): number[] {
  const sel = state.selection;
  const { from, to, empty } = sel.main;
  if (sel.ranges.length > 1 || empty || to - from > 200) return [];
  const needle = state.sliceDoc(from, to);
  if (needle.includes("\n")) return [];
  const out: number[] = [];
  for (const c = new SearchCursor(state.doc, needle); !c.next().done; ) out.push(c.value.from);
  return out;
}

/** A strip down the editor's right edge with a tick for each of the review's changes, a white
 * one for each match of the selected text, and a band for what's on screen, while there's
 * either. A click scrolls there. */
class Ruler {
  readonly canvas = document.createElement("canvas");
  matches: number[];

  constructor(readonly view: EditorView) {
    this.canvas.className = "cm-diff-ruler";
    this.canvas.onmousedown = (e) => {
      e.preventDefault();
      const r = this.canvas.getBoundingClientRect();
      const scroller = view.scrollDOM;
      const at = ((e.clientY - r.top) / r.height) * scroller.scrollHeight;
      scroller.scrollTo({ top: at - scroller.clientHeight / 2 });
    };
    this.matches = selectionMatches(view.state);
    view.dom.appendChild(this.canvas);
    view.scrollDOM.addEventListener("scroll", this.draw);
    this.draw();
  }

  update(u: ViewUpdate) {
    if (u.selectionSet || u.docChanged) this.matches = selectionMatches(u.state);
    if (u.docChanged || u.geometryChanged || u.viewportChanged || u.transactions.length)
      this.draw();
  }

  draw = () => {
    const { view, canvas } = this;
    const blocks = shownBlocks(view.state);
    // Only changes make room for the ruler; a selection's matches lie over the text's edge.
    view.dom.classList.toggle("cm-has-ruler", blocks.length > 0);
    view.dom.classList.toggle("cm-ruler-on", blocks.length > 0 || this.matches.length > 0);
    if (!blocks.length && !this.matches.length) return;
    const scroller = view.scrollDOM;
    const height = scroller.clientHeight;
    const dpr = window.devicePixelRatio || 1;
    canvas.style.top = `${scroller.offsetTop}px`;
    canvas.style.height = `${height}px`;
    canvas.width = WIDTH * dpr;
    canvas.height = height * dpr;
    const g = canvas.getContext("2d");
    if (!g) return;
    g.scale(dpr, dpr);
    const total = Math.max(scroller.scrollHeight, 1);
    const y = (top: number) => (top / total) * height;
    g.fillStyle = "rgb(255 255 255 / 9%)";
    g.fillRect(0, y(scroller.scrollTop), WIDTH, Math.max(y(scroller.clientHeight), 4));
    const css = getComputedStyle(view.dom);
    const doc = view.state.doc;
    for (const b of blocks) {
      g.fillStyle = css.getPropertyValue(COLOR[b.kind]);
      if (b.kind === "removed") {
        const at = b.from > doc.lines ? doc.length : doc.line(b.from).from;
        g.fillRect(1, y(view.lineBlockAt(at).top) - 1.5, WIDTH - 2, 3);
        continue;
      }
      const top = view.lineBlockAt(doc.line(b.from).from).top;
      const bottom = view.lineBlockAt(doc.line(b.to).from).bottom;
      g.fillRect(2, y(top), WIDTH - 4, Math.max(y(bottom) - y(top), 3));
    }
    g.fillStyle = "#fff";
    for (const at of this.matches) g.fillRect(1, y(view.lineBlockAt(at).top), WIDTH - 2, 2);
  };

  destroy() {
    this.view.scrollDOM.removeEventListener("scroll", this.draw);
    this.view.dom.classList.remove("cm-has-ruler", "cm-ruler-on");
    this.canvas.remove();
  }
}

export const diffRuler = ViewPlugin.fromClass(Ruler);

/** Keeps the editor's text clear of the ruler. */
export const rulerTheme = EditorView.theme({
  "&.cm-has-ruler .cm-scroller": { marginRight: `${WIDTH}px` },
});
