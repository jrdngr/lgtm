import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { shownBlocks } from "./diffGutter";

const WIDTH = 10;

/** The color of a block's ticks, from the app's CSS variables. */
const COLOR = { added: "--added", changed: "--amber", removed: "--error" } as const;

/** A strip down the editor's right edge with a tick for each of the review's changes and a
 * band for what's on screen, while the file has changes. A click scrolls there. */
class Ruler {
  readonly canvas = document.createElement("canvas");

  constructor(readonly view: EditorView) {
    this.canvas.className = "cm-diff-ruler";
    this.canvas.onmousedown = (e) => {
      e.preventDefault();
      const r = this.canvas.getBoundingClientRect();
      const scroller = view.scrollDOM;
      const at = ((e.clientY - r.top) / r.height) * scroller.scrollHeight;
      scroller.scrollTo({ top: at - scroller.clientHeight / 2 });
    };
    view.dom.appendChild(this.canvas);
    view.scrollDOM.addEventListener("scroll", this.draw);
    this.draw();
  }

  update(u: ViewUpdate) {
    if (u.docChanged || u.geometryChanged || u.viewportChanged || u.transactions.length)
      this.draw();
  }

  draw = () => {
    const { view, canvas } = this;
    const blocks = shownBlocks(view.state);
    view.dom.classList.toggle("cm-has-ruler", blocks.length > 0);
    if (!blocks.length) return;
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
  };

  destroy() {
    this.view.scrollDOM.removeEventListener("scroll", this.draw);
    this.view.dom.classList.remove("cm-has-ruler");
    this.canvas.remove();
  }
}

export const diffRuler = ViewPlugin.fromClass(Ruler);

/** Keeps the editor's text clear of the ruler. */
export const rulerTheme = EditorView.theme({
  "&.cm-has-ruler .cm-scroller": { marginRight: `${WIDTH}px` },
});
