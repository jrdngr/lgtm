import { type EditorState, StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";

/** Where the agent box sits: the end of a line, and the element React renders it into. */
interface Placed {
  pos: number;
  dom: HTMLElement;
}

/** A block under a line holding an element the editor doesn't manage. Events in it are left to
 * the element, so its text box types and clicks on its own. */
class BoxWidget extends WidgetType {
  constructor(readonly dom: HTMLElement) {
    super();
  }
  eq(other: BoxWidget) {
    return other.dom === this.dom;
  }
  toDOM() {
    return this.dom;
  }
}

/** Shows `dom` under the line at `pos`, or with null takes it away. */
export const placeAgentBox = StateEffect.define<Placed | null>();

/** The agent box's place, kept at the end of its line through edits and reloads. */
export const agentBoxField = StateField.define<Placed | null>({
  create: () => null,
  update(placed, tr) {
    for (const e of tr.effects) if (e.is(placeAgentBox)) placed = e.value;
    if (!placed || !tr.docChanged) return placed;
    const pos = tr.newDoc.lineAt(tr.changes.mapPos(placed.pos, 1)).to;
    return { ...placed, pos };
  },
  provide: (f) =>
    EditorView.decorations.from(f, (placed) =>
      placed
        ? Decoration.set(
            Decoration.widget({ widget: new BoxWidget(placed.dom), block: true, side: 1 }).range(
              placed.pos,
            ),
          )
        : Decoration.none,
    ),
});

/** The selection's lines, from 1, and their text: the cursor's line when nothing is selected.
 * A selection ending at the start of a line stops on the line before. */
export function selectedLines(state: EditorState) {
  const { from, to } = state.selection.main;
  const doc = state.doc;
  const first = doc.lineAt(from);
  let last = doc.lineAt(to);
  if (to > from && to === last.from) last = doc.line(last.number - 1);
  const text = from === to ? first.text : state.sliceDoc(from, to);
  return { start: first.number, end: last.number, text, pos: last.to };
}
