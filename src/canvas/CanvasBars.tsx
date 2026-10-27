import { useEffect, useRef, useState } from "react";
import type { AgentPass, CanvasShape, EdgeStyle, Swatch } from "../bindings";
import type { Pt } from "../system/edgePaths";
import { SWATCHES, swatchHex } from "./palette";
import { idsOf, parseRef } from "./refs";
import { defaultStyle, type Tool } from "./tools";
import type { useCanvasEdit } from "./useCanvasEdit";
import { type Canvas, isShape, withView } from "./view";

type Ed = ReturnType<typeof useCanvasEdit>;

function Swatches({ on, onPick }: { on?: Swatch | null; onPick(c: Swatch | null): void }) {
  return (
    <span className="cv-swatches">
      <button
        type="button"
        aria-label="No Color"
        className={`cv-sw none${on === null ? " on" : ""}`}
        onClick={() => onPick(null)}
      />
      {SWATCHES.map((c) => (
        <button
          key={c}
          type="button"
          aria-label={c}
          className={`cv-sw${on === c ? " on" : ""}`}
          style={{ background: swatchHex(c) }}
          onClick={() => onPick(c)}
        />
      ))}
    </span>
  );
}

function Seg<T extends string>(p: { items: [T, string][]; on?: T; onPick(v: T): void }) {
  return (
    <span className="cv-seg">
      {p.items.map(([v, label]) => (
        <button
          key={v}
          type="button"
          className={p.on === v ? "on" : undefined}
          onClick={() => p.onPick(v)}
        >
          {label}
        </button>
      ))}
    </span>
  );
}

const Sep = () => <span className="cv-sep" />;

/** The one value all of `xs` share, if they share one. */
function same<T>(xs: T[]): T | undefined {
  return xs.length && xs.every((x) => x === xs[0]) ? xs[0] : undefined;
}

/** The toolbar over the selection, by what's selected. */
export function SelectionBar(p: {
  ed: Ed;
  canvas: Canvas;
  commit(c: Canvas): void;
  toScreen(p: Pt): Pt;
}) {
  const { ed, canvas } = p;
  const box = ed.selectionBox();
  if (!box || ed.gesture) return null;
  const v = ed.view;
  const sel = ed.selected;
  const all = idsOf(sel, "part");
  const parts = all.filter((id) => !isShape(id));
  const shapes = all.filter(isShape);
  const shapeOf = (id: string) => v.shapes.find((x) => x.id === id);
  const groups = idsOf(sel, "group");
  const notes = idsOf(sel, "note");
  const arrows = idsOf(sel, "arrow");
  const edges = idsOf(sel, "edge");
  const kinds = new Set(
    [...sel].map((r) => {
      const ref = parseRef(r);
      return ref.kind === "part" && isShape(ref.id) ? "shape" : ref.kind;
    }),
  );
  const only = kinds.size === 1 ? [...kinds][0] : undefined;

  const colorOf = [
    ...parts.map((id) => canvas.colors[id] ?? null),
    ...shapes.map((id) => shapeOf(id)?.color ?? null),
    ...groups.map((id) => v.groups.find((g) => g.id === id)?.color ?? null),
    ...notes.map((id) => v.notes.find((n) => n.id === id)?.color ?? null),
    ...arrows.map((id) => v.arrows.find((a) => a.id === id)?.color ?? null),
    ...edges.map((k) => v.edges[k]?.color ?? null),
  ];
  const styleOf = (k: string) => v.edges[k] ?? defaultStyle();
  const styleEdges = (f: (s: EdgeStyle) => EdgeStyle) => (view: typeof v) => ({
    ...view,
    edges: {
      ...view.edges,
      ...Object.fromEntries(
        edges.map((k) => [k, { ...f(view.edges[k] ?? defaultStyle()), by: "user" as const }]),
      ),
    },
  });
  const paint = (c: Swatch | null) => {
    const colors = { ...canvas.colors };
    for (const id of parts) {
      if (c) colors[id] = c;
      else delete colors[id];
    }
    const view = {
      ...v,
      shapes: v.shapes.map((x) => (shapes.includes(x.id) ? { ...x, color: c } : x)),
      groups: v.groups.map((g) => (groups.includes(g.id) ? { ...g, color: c } : g)),
      notes: v.notes.map((n) => (notes.includes(n.id) ? { ...n, color: c } : n)),
      arrows: v.arrows.map((a) => (arrows.includes(a.id) ? { ...a, color: c } : a)),
    };
    const styled = edges.length ? styleEdges((s) => ({ ...s, color: c }))(view) : view;
    p.commit(withView({ ...canvas, colors }, ed.viewKey, () => styled));
  };
  const swatches = <Swatches on={same(colorOf)} onPick={paint} />;
  const remove = (
    <button type="button" className="cv-btn" onClick={ed.removeSelection}>
      Remove
    </button>
  );

  const styleShapes = (f: (x: CanvasShape) => CanvasShape) =>
    ed.change((view) => ({
      ...view,
      shapes: view.shapes.map((x) => (shapes.includes(x.id) ? { ...f(x), by: "user" } : x)),
    }));

  let body: React.ReactNode;
  if (only === "shape") {
    body = (
      <>
        {swatches}
        <Sep />
        <Seg
          items={[
            ["fill", "Fill"],
            ["outline", "Outline"],
          ]}
          on={same(shapes.map((id) => (shapeOf(id)?.fill ? "fill" : "outline")))}
          onPick={(f) => styleShapes((x) => ({ ...x, fill: f === "fill" }))}
        />
        <Seg
          items={[
            ["solid", "Solid"],
            ["dashed", "Dashed"],
          ]}
          on={same(shapes.map((id) => (shapeOf(id)?.pattern === "solid" ? "solid" : "dashed")))}
          onPick={(pattern) => styleShapes((x) => ({ ...x, pattern }))}
        />
        <Sep />
        <button type="button" className="cv-btn" onClick={ed.groupSelection}>
          Group<kbd>⌘G</kbd>
        </button>
        {remove}
      </>
    );
  } else if (only === "group" && groups.length === 1) {
    body = (
      <>
        <GroupLabel ed={ed} id={groups[0]} />
        <Sep />
        {swatches}
        <Sep />
        <button type="button" className="cv-btn" onClick={ed.ungroupSelection}>
          Ungroup<kbd>⇧⌘G</kbd>
        </button>
      </>
    );
  } else if (only === "edge") {
    const on = edges.map(styleOf);
    body = (
      <>
        <span className="cv-name">Line</span>
        <Seg
          items={[
            ["solid", "Solid"],
            ["dashed", "Dashed"],
            ["dotted", "Dotted"],
          ]}
          on={same(on.map((s) => s.pattern))}
          onPick={(pattern) => ed.change(styleEdges((s) => ({ ...s, pattern })))}
        />
        <Sep />
        {swatches}
        <Sep />
        <Seg
          items={[
            ["thin", "Thin"],
            ["thick", "Thick"],
          ]}
          on={same(on.map((s) => s.weight))}
          onPick={(weight) => ed.change(styleEdges((s) => ({ ...s, weight })))}
        />
        <Sep />
        <Seg
          items={[
            ["curved", "Curved"],
            ["elbow", "Elbow"],
          ]}
          on={same(on.map((s) => s.route))}
          onPick={(route) => ed.change(styleEdges((s) => ({ ...s, route, bend: null })))}
        />
      </>
    );
  } else if (only === "note") {
    body = (
      <>
        <Seg
          items={[
            ["s", "S"],
            ["m", "M"],
            ["l", "L"],
          ]}
          on={same(notes.map((id) => v.notes.find((n) => n.id === id)?.size))}
          onPick={(size) =>
            ed.change((view) => ({
              ...view,
              notes: view.notes.map((n) => (notes.includes(n.id) ? { ...n, size } : n)),
            }))
          }
        />
        <Sep />
        {swatches}
        <Sep />
        {remove}
      </>
    );
  } else if (only === "arrow") {
    body = (
      <>
        {swatches}
        <Sep />
        <Seg
          items={[
            ["solid", "Solid"],
            ["dashed", "Dashed"],
          ]}
          on={same(arrows.map((id) => v.arrows.find((a) => a.id === id)?.pattern))}
          onPick={(pattern) =>
            ed.change((view) => ({
              ...view,
              arrows: view.arrows.map((a) => (arrows.includes(a.id) ? { ...a, pattern } : a)),
            }))
          }
        />
        <Sep />
        <button
          type="button"
          className="cv-btn"
          onClick={() =>
            ed.change((view) => ({
              ...view,
              arrows: view.arrows.map((a) =>
                arrows.includes(a.id)
                  ? {
                      ...a,
                      from: { target: null, at: ed.endPoint(a.from, a.to) },
                      to: { target: null, at: ed.endPoint(a.to, a.from) },
                    }
                  : a,
              ),
            }))
          }
        >
          Detach
        </button>
        {remove}
      </>
    );
  } else if (!notes.length && !arrows.length && !edges.length) {
    body = (
      <>
        {swatches}
        <Sep />
        <button type="button" className="cv-btn" onClick={ed.groupSelection}>
          Group<kbd>⌘G</kbd>
        </button>
      </>
    );
  } else {
    body = (
      <>
        {swatches}
        <Sep />
        {remove}
      </>
    );
  }

  const at = p.toScreen({ x: box.x, y: box.y });
  const below = p.toScreen({ x: box.x, y: box.y + box.h });
  const top = at.y - 56 < 8 ? below.y + 14 : at.y - 56;
  return (
    <div
      className="cv-bar"
      style={{ left: Math.max(8, at.x - 40), top }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {body}
    </div>
  );
}

/** A selected group's name, saved on Enter or leaving the field. */
function GroupLabel({ ed, id }: { ed: Ed; id: string }) {
  const label = ed.view.groups.find((g) => g.id === id)?.label ?? "";
  const [text, setText] = useState(label);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => setText(label), [label]);
  useEffect(() => {
    if (ed.renaming !== id) return;
    const t = setTimeout(() => ref.current?.focus());
    return () => clearTimeout(t);
  }, [ed.renaming, id]);
  const save = () => {
    ed.setRenaming(undefined);
    const t = text.trim();
    if (!t || t === label) return setText(label);
    ed.change((view) => ({
      ...view,
      groups: view.groups.map((g) => (g.id === id ? { ...g, label: t, by: "user" } : g)),
    }));
  };
  return (
    <input
      className="cv-label"
      aria-label="Group Name"
      value={text}
      ref={ref}
      onFocus={(e) => e.target.select()}
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          setText(label);
          ed.setRenaming(undefined);
        }
      }}
    />
  );
}

const icon = (d: React.ReactNode, props: React.SVGProps<SVGSVGElement> = {}) => (
  <svg
    width="13"
    height="13"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.6"
    strokeLinecap="round"
    aria-hidden="true"
    {...props}
  >
    {d}
  </svg>
);

const TOOLS: [Tool, string, string, React.ReactNode][] = [
  [
    "select",
    "Select",
    "v",
    icon(<path d="M3 2l10 6-4.5 1L6 14z" fill="currentColor" stroke="none" />),
  ],
  ["rect", "Rectangle", "r", icon(<rect x="2.5" y="3.5" width="11" height="9" rx="1.5" />)],
  ["ellipse", "Ellipse", "o", icon(<ellipse cx="8" cy="8" rx="6" ry="4.5" />)],
  ["diamond", "Diamond", "d", icon(<path d="M8 2l6 6-6 6-6-6z" />)],
  ["arrow", "Arrow", "a", icon(<path d="M3 13L13 3M7 3h6v6" />, { strokeWidth: 1.8 })],
  ["line", "Line", "l", icon(<path d="M3 13L13 3" />, { strokeWidth: 1.8 })],
  [
    "text",
    "Text",
    "t",
    <b key="t" className="cv-t">
      T
    </b>,
  ],
  ["note", "Note", "n", icon(<path d="M3 3h10v6l-4 4H3z M9 13V9h4" />)],
  [
    "frame",
    "Frame",
    "f",
    icon(<rect x="2" y="2.5" width="12" height="11" rx="2.5" />, { strokeDasharray: "2.5 2" }),
  ],
];

/** The editing tools along the canvas's bottom, then the tags. */
export function EditTools({ ed, onResetLayout }: { ed: Ed; onResetLayout?(): void }) {
  const placed = new Set(Object.keys(ed.view.tags));
  return (
    <div className="cv-tools" onPointerDown={(e) => e.stopPropagation()}>
      {TOOLS.map(([t, label, key, glyph]) => (
        <button
          key={t}
          type="button"
          className={`cv-btn${ed.tool === t ? " on" : ""}`}
          aria-pressed={ed.tool === t}
          aria-label={label}
          onClick={() => ed.setTool(t)}
        >
          {glyph}
          <span className="cv-tool-name">{label}</span>
          <kbd>{key}</kbd>
        </button>
      ))}
      <Sep />
      <span className="cv-tray">
        {"0123456789".split("").map((d) => (
          <button
            key={d}
            type="button"
            aria-label={`Tag ${d}`}
            className={`cv-chip${placed.has(d) ? " placed" : ""}`}
            onPointerDown={(e) => ed.press.tag(d, e)}
          >
            {d}
          </button>
        ))}
      </span>
      <button
        type="button"
        className="cv-btn"
        disabled={!placed.size && !Object.keys(ed.view.agentTags).length}
        onClick={ed.clearTags}
      >
        Clear tags
      </button>
      {onResetLayout && (
        <>
          <Sep />
          <button
            type="button"
            className="cv-btn"
            disabled={!ed.view.frozen}
            onClick={onResetLayout}
          >
            Reset Layout
          </button>
        </>
      )}
    </div>
  );
}

/** The last agent pass, with its undo. */
export function AgentStrip(p: { pass: AgentPass; onUndo(): void; onShow(): void }) {
  return (
    <div className="cv-agent">
      <span className="cv-agent-dot" />
      <b>Agent changed {p.pass.changed}</b>
      <button type="button" className="sys-tool" onClick={p.onUndo}>
        Undo
      </button>
      <button type="button" className="sys-tool" onClick={p.onShow}>
        Show
      </button>
    </div>
  );
}
