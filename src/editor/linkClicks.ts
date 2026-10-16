import { type Extension, StateEffect, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";
import { cmdClick, trackCmd } from "../ui/cmdHeld";
import { type LineLink, linksIn } from "./links";

/** The link under the pointer, underlined while ⌘ is held. */
const hot = Decoration.mark({ class: "cm-link-hot" });
const setHover = StateEffect.define<LineLink | null>();

const hover = StateField.define<{ link: LineLink | null; deco: DecorationSet }>({
  create: () => ({ link: null, deco: Decoration.none }),
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setHover)) {
        const link = e.value;
        return {
          link,
          deco: link ? Decoration.set([hot.range(link.from, link.to)]) : Decoration.none,
        };
      }
    }
    return tr.docChanged ? { link: null, deco: Decoration.none } : value;
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.deco),
});

/** The link at the pointer, with document positions. */
function linkAt(view: EditorView, e: MouseEvent): LineLink | null {
  const pos = view.posAtCoords({ x: e.clientX, y: e.clientY }, false);
  if (pos === null) return null;
  const line = view.state.doc.lineAt(pos);
  const at = pos - line.from;
  const link = linksIn(line.text).find((l) => l.from <= at && at < l.to);
  return link ? { ...link, from: line.from + link.from, to: line.from + link.to } : null;
}

function show(view: EditorView, link: LineLink | null) {
  const now = view.state.field(hover).link;
  if (now?.from === link?.from && now?.to === link?.to) return;
  view.dispatch({ effects: setHover.of(link) });
}

/** ⌘-click on a markdown link calls `open` with where it goes. */
export function linkClicks(open: (href: string) => void): Extension {
  trackCmd();
  return [
    hover,
    EditorView.domEventHandlers({
      mousemove: (e, view) => show(view, linkAt(view, e)),
      mouseleave: (_, view) => show(view, null),
      mousedown(e, view) {
        if (e.button !== 0 || !cmdClick(e)) return false;
        const link = linkAt(view, e);
        if (!link) return false;
        e.preventDefault();
        open(link.href);
        return true;
      },
    }),
  ];
}
