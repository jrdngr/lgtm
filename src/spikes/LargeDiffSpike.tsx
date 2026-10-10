import {
  cursorLineDown,
  cursorLineUp,
  selectAll,
  selectLineDown,
  selectLineUp,
} from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
import {
  EditorState,
  type Extension,
  RangeSetBuilder,
  StateEffect,
  StateField,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  GutterMarker,
  gutter,
  keymap,
  lineNumbers,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import { useEffect, useRef, useState } from "react";

// Measures CodeMirror with a 5,000-row diff: row tints, a mark gutter, j/k movement, and
// marking. Synthetic content stands in for a real jj diff.

type Kind = "add" | "del" | "ctx" | "hunk";

const TEMPLATES = [
  "export function handle{n}(req: Request, ctx: Context): Promise<Response> {",
  "  const user = await ctx.users.get(req.params.id{n});",
  "  if (!user) throw new NotFound('user missing: ' + req.params.id);",
  "  const items = user.items.filter((i) => i.active && i.weight > {n});",
  "  return json({ id: user.id, count: items.length, total: sum(items) });",
  "}",
  "",
];

function generate(rows: number) {
  const lines: string[] = [];
  const kinds: Kind[] = [];
  for (let i = 0; i < rows; i++) {
    if (i % 40 === 0) {
      lines.push(`@@ -${i},30 +${i},34 @@ function handle${i}`);
      kinds.push("hunk");
      continue;
    }
    const r = (i * 7919) % 10;
    kinds.push(r < 3 ? "add" : r < 5 ? "del" : "ctx");
    lines.push(TEMPLATES[i % TEMPLATES.length].replaceAll("{n}", String(i)));
  }
  return { text: lines.join("\n"), kinds };
}

const markReviewed = StateEffect.define<number[]>();

/** 1-based line numbers marked reviewed. */
const reviewed = StateField.define<ReadonlySet<number>>({
  create: () => new Set(),
  update(value, tr) {
    let next = value;
    for (const e of tr.effects) {
      if (!e.is(markReviewed)) continue;
      const added = e.value.filter((n) => !next.has(n));
      if (added.length) next = new Set([...next, ...added]);
    }
    return next;
  },
});

const rowClass = (kind: Kind, isReviewed: boolean) =>
  `row-${kind}${isReviewed ? " row-reviewed" : ""}`;

/** Decorates only the visible rows, so cost tracks the viewport, not the diff size. */
function rowTints(kinds: Kind[]) {
  const build = (view: EditorView): DecorationSet => {
    const marks = view.state.field(reviewed);
    const builder = new RangeSetBuilder<Decoration>();
    for (const { from, to } of view.visibleRanges) {
      for (let pos = from; pos <= to; ) {
        const line = view.state.doc.lineAt(pos);
        const cls = rowClass(kinds[line.number - 1], marks.has(line.number));
        builder.add(line.from, line.from, Decoration.line({ class: cls }));
        pos = line.to + 1;
      }
    }
    return builder.finish();
  };
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = build(view);
      }
      update(u: ViewUpdate) {
        if (u.viewportChanged || u.startState.field(reviewed) !== u.state.field(reviewed)) {
          this.decorations = build(u.view);
        }
      }
    },
    { decorations: (v) => v.decorations },
  );
}

class StripMarker extends GutterMarker {
  elementClass = "mark-reviewed";
}
const strip = new StripMarker();

const markGutter = gutter({
  class: "mark-gutter",
  lineMarker: (view, line) =>
    view.state.field(reviewed).has(view.state.doc.lineAt(line.from).number) ? strip : null,
  lineMarkerChange: (u) => u.startState.field(reviewed) !== u.state.field(reviewed),
});

function markSelection(kinds: Kind[], onTimed: (ms: number) => void) {
  return (view: EditorView) => {
    const start = performance.now();
    const { doc } = view.state;
    const lines: number[] = [];
    for (const r of view.state.selection.ranges) {
      const last = doc.lineAt(r.to).number;
      for (let n = doc.lineAt(r.from).number; n <= last; n++) {
        if (kinds[n - 1] === "add" || kinds[n - 1] === "del") lines.push(n);
      }
    }
    view.dispatch({ effects: markReviewed.of(lines) });
    requestAnimationFrame(() => onTimed(performance.now() - start));
    return true;
  };
}

/** Tracks the worst frame while scrolling. */
function scrollMeter(onFrame: (worstMs: number) => void): Extension {
  let worst = 0;
  let last = 0;
  let idleTimer: number | undefined;
  let frame: number | undefined;
  const tick = (t: number) => {
    if (last) worst = Math.max(worst, t - last);
    last = t;
    frame = requestAnimationFrame(tick);
  };
  return EditorView.domEventHandlers({
    scroll() {
      if (frame === undefined) {
        worst = 0;
        last = 0;
        frame = requestAnimationFrame(tick);
      }
      clearTimeout(idleTimer);
      idleTimer = window.setTimeout(() => {
        if (frame !== undefined) cancelAnimationFrame(frame);
        frame = undefined;
        onFrame(worst);
      }, 300);
    },
  });
}

const ROWS = 5000;

export function LargeDiffSpike() {
  const ref = useRef<HTMLDivElement>(null);
  const [stats, setStats] = useState({ reviewed: 0, changed: 0 });
  const [markMs, setMarkMs] = useState<number>();
  const [worstFrameMs, setWorstFrameMs] = useState<number>();

  useEffect(() => {
    const { text, kinds } = generate(ROWS);
    const changed = kinds.filter((k) => k === "add" || k === "del").length;
    const mark = markSelection(kinds, setMarkMs);
    const view = new EditorView({
      parent: ref.current as HTMLDivElement,
      state: EditorState.create({
        doc: text,
        extensions: [
          EditorState.readOnly.of(true),
          lineNumbers(),
          markGutter,
          javascript({ typescript: true }),
          syntaxHighlighting(defaultHighlightStyle),
          reviewed,
          rowTints(kinds),
          scrollMeter(setWorstFrameMs),
          EditorView.domEventHandlers({
            // Mark keys ignore key repeat.
            keydown: (e) => e.key === " " && e.repeat,
          }),
          keymap.of([
            { key: "j", run: cursorLineDown },
            { key: "k", run: cursorLineUp },
            { key: "J", run: selectLineDown },
            { key: "K", run: selectLineUp },
            { key: "Space", run: mark },
            { key: "Mod-a", run: selectAll },
          ]),
          EditorView.updateListener.of((u) => {
            if (u.startState.field(reviewed) !== u.state.field(reviewed)) {
              setStats({ reviewed: u.state.field(reviewed).size, changed });
            }
          }),
        ],
      }),
    });
    setStats({ reviewed: 0, changed });
    view.focus();
    return () => view.destroy();
  }, []);

  return (
    <div className="spike">
      <div className="spike-bar">
        <span>
          {ROWS} rows · {stats.reviewed}/{stats.changed} changed rows reviewed
        </span>
        <span>last mark: {markMs === undefined ? "-" : `${markMs.toFixed(1)} ms`}</span>
        <span>
          worst frame while scrolling:{" "}
          {worstFrameMs === undefined ? "-" : `${worstFrameMs.toFixed(1)} ms`}
        </span>
        <span className="hint">j/k move · J/K extend · space marks · ⌘A then space marks all</span>
      </div>
      <div ref={ref} className="diff" />
    </div>
  );
}
