import { useEffect, useRef, useState } from "react";
import type { AgentPass, EdgeStyle, Swatch } from "../../bindings";
import type { Pt } from "../../system/edgePaths";
import { SWATCHES, swatchHex } from "./palette";
import { idsOf, parseRef } from "./refs";
import { defaultStyle, type Tool, type useCanvasEdit } from "./useCanvasEdit";
import { type Canvas, withView } from "./view";

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
  const parts = idsOf(sel, "part");
  const groups = idsOf(sel, "group");
  const notes = idsOf(sel, "note");
  const arrows = idsOf(sel, "arrow");
  const edges = idsOf(sel, "edge");
  const kinds = new Set([...sel].map((r) => parseRef(r).kind));
  const only = kinds.size === 1 ? [...kinds][0] : undefined;

  const colorOf = [
    ...parts.map((id) => canvas.colors[id] ?? null),
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

  let body: React.ReactNode;
  if (only === "group" && groups.length === 1) {
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

const SelectIcon = () => (
  <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
    <path d="M3 2l10 6-4.5 1L6 14z" />
  </svg>
);
const ArrowIcon = () => (
  <svg
    width="13"
    height="13"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    aria-hidden="true"
  >
    <path d="M3 13L13 3M7 3h6v6" />
  </svg>
);

/** The editing tools along the canvas's bottom. */
export function EditTools({ ed }: { ed: Ed }) {
  const tool = (t: Tool, icon: React.ReactNode, label: string, key: string) => (
    <button
      type="button"
      className={`cv-btn${ed.tool === t ? " on" : ""}`}
      aria-pressed={ed.tool === t}
      onClick={() => ed.setTool(t)}
    >
      {icon}
      {label}
      <kbd>{key}</kbd>
    </button>
  );
  const groupable = idsOf(ed.selected, "part").length + idsOf(ed.selected, "group").length > 0;
  return (
    <div className="cv-tools" onPointerDown={(e) => e.stopPropagation()}>
      {tool("select", <SelectIcon />, "Select", "v")}
      {tool("text", <b className="cv-t">T</b>, "Text", "t")}
      {tool("arrow", <ArrowIcon />, "Arrow", "a")}
      <Sep />
      <button type="button" className="cv-btn" disabled={!groupable} onClick={ed.groupSelection}>
        Group<kbd>⌘G</kbd>
      </button>
      <Sep />
      <button type="button" className="cv-btn" disabled={!ed.view.frozen} onClick={ed.resetLayout}>
        Reset Layout
      </button>
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
