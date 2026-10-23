import { type EditorState, type Range, RangeSet, StateEffect, StateField } from "@codemirror/state";
import {
  type EditorView,
  GutterMarker,
  gutter,
  showTooltip,
  type Tooltip,
  ViewPlugin,
} from "@codemirror/view";
import { type DiffBlock, lineMap, mapBlocks } from "./diffBlocks";

/** The review's changes to a file: its changed runs, on the lines of its new side. */
export interface DiffSource {
  blocks: DiffBlock[];
  newSide: string[];
}

/** Puts the review's changes to the file in the gutter, or clears them. */
export const setDiff = StateEffect.define<DiffSource | undefined>();
/** Matches the changes to the editor's text again, after edits. */
const remap = StateEffect.define<null>();
/** Edits wait this long before the gutter follows them. */
const SETTLE_MS = 300;
/** Forgets the block whose removed lines are these, once they're put back. */
const dropBlock = StateEffect.define<string[]>();
/** Opens the removed lines of block `i`, or closes them. */
const togglePeek = StateEffect.define<number>();

class Bar extends GutterMarker {
  constructor(readonly kind: "added" | "changed") {
    super();
  }
  eq(other: Bar) {
    return other.kind === this.kind;
  }
  toDOM() {
    const el = document.createElement("div");
    el.className = `cm-diff-bar ${this.kind}`;
    return el;
  }
}

/** Where lines were removed: under its line, or over the first line. */
class Wedge extends GutterMarker {
  constructor(readonly top: boolean) {
    super();
  }
  eq(other: Wedge) {
    return other.top === this.top;
  }
  toDOM() {
    const el = document.createElement("div");
    el.className = `cm-diff-wedge${this.top ? " top" : ""}`;
    return el;
  }
}

interface Diff {
  source?: DiffSource;
  /** The source's blocks on the editor's lines. */
  blocks: DiffBlock[];
  markers: RangeSet<GutterMarker>;
  /** The block whose removed lines show. */
  peek?: number;
}

function markersOf(state: EditorState, blocks: DiffBlock[]) {
  const doc = state.doc;
  const marks: Range<GutterMarker>[] = [];
  for (const b of blocks) {
    if (b.kind === "removed") {
      const line = doc.line(Math.max(b.to, 1));
      marks.push(new Wedge(b.to === 0).range(line.from));
    } else
      for (let n = b.from; n <= b.to && n <= doc.lines; n++)
        marks.push(new Bar(b.kind).range(doc.line(n).from));
  }
  return RangeSet.of(marks, true);
}

/** The block a click on gutter line `n` opens: lines removed next to it, since the wedge
 * straddles two lines, else the changed block it's in. */
function blockAt(blocks: DiffBlock[], n: number) {
  const near = (b: DiffBlock) => n === Math.max(b.to, 1) || n === b.from;
  const removed = blocks.findIndex((b) => b.kind === "removed" && near(b));
  if (removed >= 0) return removed;
  return blocks.findIndex((b) => b.kind === "changed" && b.from <= n && n <= b.to);
}

function restore(view: EditorView, b: DiffBlock) {
  const doc = view.state.doc;
  const text = b.removed.join(view.state.lineBreak);
  const change =
    b.kind === "removed"
      ? b.from > doc.lines
        ? { from: doc.length, insert: view.state.lineBreak + text }
        : { from: doc.line(b.from).from, insert: text + view.state.lineBreak }
      : { from: doc.line(b.from).from, to: doc.line(b.to).to, insert: text };
  view.dispatch({ changes: change, effects: dropBlock.of(b.removed) });
  view.focus();
}

function peekTooltip(state: EditorState, d: Diff): Tooltip | null {
  const b = d.peek === undefined ? undefined : d.blocks[d.peek];
  if (!b) return null;
  const line = state.doc.line(Math.min(Math.max(b.to, 1), state.doc.lines));
  return {
    pos: line.from,
    above: b.to === 0,
    create(view) {
      const dom = document.createElement("div");
      dom.className = "cm-diff-peek";
      for (const t of b.removed) {
        const row = document.createElement("div");
        row.className = "cm-diff-peek-row";
        const sign = document.createElement("b");
        sign.textContent = "−";
        row.append(sign, t);
        dom.append(row);
      }
      const bar = document.createElement("div");
      bar.className = "cm-diff-peek-bar";
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = "Restore";
      button.onmousedown = (e) => e.preventDefault();
      button.onclick = () => restore(view, b);
      bar.append(button);
      dom.append(bar);
      return { dom };
    },
  };
}

/** The source's blocks matched to the editor's text, line by line, so edits since keep the
 * marks on their lines and drop the ones they touch. */
function matched(state: EditorState, source?: DiffSource): Diff {
  if (!source) return { blocks: [], markers: RangeSet.empty };
  const lines = state.doc.toString().split(state.lineBreak);
  const blocks = mapBlocks(source.blocks, lineMap(source.newSide, lines), state.doc.lines);
  return { source, blocks, markers: markersOf(state, blocks) };
}

const field = StateField.define<Diff>({
  create: () => ({ blocks: [], markers: RangeSet.empty }),
  update(value, tr) {
    let next: Diff = tr.docChanged
      ? { source: value.source, blocks: value.blocks, markers: value.markers.map(tr.changes) }
      : value;
    for (const e of tr.effects) {
      if (e.is(setDiff)) next = matched(tr.state, e.value);
      if (e.is(remap)) next = matched(tr.state, next.source);
      if (e.is(dropBlock) && next.source) {
        const blocks = next.source.blocks.filter((b) => b.removed !== e.value);
        next = matched(tr.state, { ...next.source, blocks });
      }
      if (e.is(togglePeek)) next = { ...next, peek: next.peek === e.value ? undefined : e.value };
    }
    return next;
  },
  provide: (f) => showTooltip.compute([f], (s) => peekTooltip(s, s.field(f))),
});

/** The review's changes to the file as bars left of the line numbers: green for added lines,
 * amber for changed ones, and a red wedge where lines were removed, which opens them.
 * Goes before `lineNumbers()`. */
export const diffGutter = [
  field,
  ViewPlugin.define((view) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    return {
      update(u) {
        if (!u.docChanged || !u.state.field(field).source) return;
        clearTimeout(timer);
        timer = setTimeout(() => view.dispatch({ effects: remap.of(null) }), SETTLE_MS);
      },
      destroy: () => clearTimeout(timer),
    };
  }),
  gutter({
    class: "cm-diff-gutter",
    markers: (v) => v.state.field(field).markers,
    domEventHandlers: {
      mousedown(view, line) {
        const d = view.state.field(field);
        const i = blockAt(d.blocks, view.state.doc.lineAt(line.from).number);
        if (i < 0) return false;
        view.dispatch({ effects: togglePeek.of(i) });
        return true;
      },
    },
  }),
];

/** The review's changed runs on the editor's lines. */
export const shownBlocks = (state: EditorState) => state.field(field, false)?.blocks ?? [];
