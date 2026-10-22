import { type EditorState, RangeSet, StateEffect, StateField } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  GutterMarker,
  gutter,
  WidgetType,
} from "@codemirror/view";
import { createRoot, type Root } from "react-dom/client";
import type { SymbolInfoDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { InfoBlock } from "./InfoBlock";
import { infoKey, setFolded } from "./store";

interface Shown {
  ws: WorkspaceName;
  infos: SymbolInfoDto[];
  folded: ReadonlySet<string>;
}

/** Puts the file's Symbol Information in an editor. */
export const showInfos = StateEffect.define<Shown>();

class BlockWidget extends WidgetType {
  private root?: Root;

  constructor(
    readonly ws: WorkspaceName,
    readonly info: SymbolInfoDto,
  ) {
    super();
  }

  eq(other: BlockWidget) {
    return other.ws === this.ws && JSON.stringify(other.info) === JSON.stringify(this.info);
  }

  toDOM() {
    const el = document.createElement("div");
    el.className = "cm-si";
    this.root = createRoot(el);
    this.root.render(<InfoBlock ws={this.ws} info={this.info} />);
    return el;
  }

  destroy() {
    const root = this.root;
    // Unmounting while CodeMirror updates its DOM can land inside a React render.
    queueMicrotask(() => root?.unmount());
  }
}

class IconMarker extends GutterMarker {
  constructor(readonly info: SymbolInfoDto) {
    super();
  }

  eq(other: IconMarker) {
    return infoKey(other.info) === infoKey(this.info) && other.info.working === this.info.working;
  }

  toDOM() {
    const b = document.createElement("button");
    b.type = "button";
    b.className = `si-icon${this.info.working ? " busy" : ""}`;
    b.textContent = "i";
    b.onmousedown = (e) => e.stopPropagation();
    b.onclick = () => setFolded(infoKey(this.info), false);
    return b;
  }
}

/** The line each info sits under, or undefined past the end of an edited file. */
function lineOf(state: EditorState, info: SymbolInfoDto) {
  return info.line <= state.doc.lines ? state.doc.line(info.line) : undefined;
}

function build(state: EditorState, s: Shown) {
  const blocks = [];
  const icons = [];
  for (const info of [...s.infos].sort((a, b) => a.line - b.line)) {
    const line = lineOf(state, info);
    if (!line) continue;
    if (s.folded.has(infoKey(info))) icons.push(new IconMarker(info).range(line.from));
    else
      blocks.push(
        Decoration.widget({ widget: new BlockWidget(s.ws, info), block: true, side: 1 }).range(
          line.to,
        ),
      );
  }
  return { blocks: Decoration.set(blocks), icons: RangeSet.of(icons) };
}

const field = StateField.define<{ blocks: DecorationSet; icons: RangeSet<GutterMarker> }>({
  create: () => ({ blocks: Decoration.none, icons: RangeSet.empty }),
  update(value, tr) {
    let next = { blocks: value.blocks.map(tr.changes), icons: value.icons.map(tr.changes) };
    for (const e of tr.effects) if (e.is(showInfos)) next = build(tr.state, e.value);
    return next;
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.blocks),
});

/** Symbol Information under its definitions, folded to icons in a gutter left of the line
 * numbers. Goes before `lineNumbers()`. */
export const symbolInfo = [
  field,
  gutter({
    class: "cm-si-gutter",
    markers: (v) => v.state.field(field).icons,
  }),
];
