import {
  type Extension,
  RangeSetBuilder,
  StateEffect,
  StateField,
  type Text,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import { HIGHLIGHT_NAMES } from "../bindings";
import { commands } from "../ipc";

type Segment = [number | null, number];

/** The diff view's `hl-*` classes, so code looks the same in both. */
const marks = HIGHLIGHT_NAMES.map((n) => Decoration.mark({ class: `hl-${n.replace(".", "-")}` }));

const setHighlights = StateEffect.define<DecorationSet>();

const highlights = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    for (const e of tr.effects) if (e.is(setHighlights)) return e.value;
    // Old colors follow edits until the backend's next pass lands.
    return deco.map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** Decorations for per-line segments, measured in UTF-16 units as CodeMirror is. */
export function decorate(doc: Text, lines: Segment[][]): DecorationSet {
  const b = new RangeSetBuilder<Decoration>();
  lines.slice(0, doc.lines).forEach((segs, i) => {
    const line = doc.line(i + 1);
    let pos = line.from;
    for (const [cls, len] of segs) {
      const end = Math.min(pos + len, line.to);
      if (cls !== null && end > pos) b.add(pos, end, marks[cls]);
      pos = end;
    }
  });
  return b.finish();
}

const DEBOUNCE_MS = 120;

/** Colors the document with the backend's tree-sitter highlighting of `path`. */
export function treeSitter(path: string): Extension {
  const plugin = ViewPlugin.fromClass(
    class {
      timer = 0;
      gone = false;
      /** No grammar for the path, so there's nothing to ask for. */
      plain = false;

      constructor(readonly view: EditorView) {
        this.request();
      }

      update(u: ViewUpdate) {
        if (!u.docChanged || this.plain) return;
        clearTimeout(this.timer);
        this.timer = window.setTimeout(() => this.request(), DEBOUNCE_MS);
      }

      async request() {
        const doc = this.view.state.doc;
        const lines = await commands.highlightText(path, doc.toString());
        // A later edit has its own request coming.
        if (this.gone || this.view.state.doc !== doc) return;
        if (lines.length === 0) this.plain = true;
        this.view.dispatch({ effects: setHighlights.of(decorate(doc, lines)) });
      }

      destroy() {
        this.gone = true;
        clearTimeout(this.timer);
      }
    },
  );
  return [highlights, plugin];
}
