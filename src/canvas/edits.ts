import type { CanvasShape, Pt, ShapeKind } from "../bindings";
import type { Box } from "../system/layout";
import { moveInto } from "./canvasGroups";
import { idsOf } from "./refs";
import type { View } from "./view";

/** A shape drawn with a click instead of a drag. */
export const SHAPE_W = 160;
export const SHAPE_H = 64;
/** The smallest a shape gets. */
export const SHAPE_MIN = 20;

/** The box a drag from `a` to `b` draws, or with no drag, a default-sized one at `a`. */
export function dragBox(a: Pt, b: Pt, dragged: boolean): Box {
  if (!dragged) return { x: Math.round(a.x), y: Math.round(a.y), w: SHAPE_W, h: SHAPE_H };
  return {
    x: Math.round(Math.min(a.x, b.x)),
    y: Math.round(Math.min(a.y, b.y)),
    w: Math.max(SHAPE_MIN, Math.round(Math.abs(b.x - a.x))),
    h: Math.max(SHAPE_MIN, Math.round(Math.abs(b.y - a.y))),
  };
}

export const newShape = (id: string, kind: ShapeKind, b: Box): CanvasShape => ({
  id,
  kind,
  at: { x: b.x, y: b.y },
  w: b.w,
  h: b.h,
  text: "",
  color: null,
  fill: true,
  pattern: "solid",
  by: "user",
});

/** `from` resized by dragging its `corner` (0 top left, 1 top right, 2 bottom left, 3 bottom
 * right) by (`dx`, `dy`). The opposite corner stays put, and it stops at the smallest size. */
export function resized(from: Box, corner: number, dx: number, dy: number): Box {
  const left = corner === 0 || corner === 2;
  const top = corner === 0 || corner === 1;
  const x1 = left ? from.x + dx : from.x;
  const x2 = left ? from.x + from.w : from.x + from.w + dx;
  const y1 = top ? from.y + dy : from.y;
  const y2 = top ? from.y + from.h : from.y + from.h + dy;
  const w = Math.max(SHAPE_MIN, Math.round(x2 - x1));
  const h = Math.max(SHAPE_MIN, Math.round(y2 - y1));
  return {
    x: Math.round(left ? Math.min(x1, x2 - SHAPE_MIN) : from.x),
    y: Math.round(top ? Math.min(y1, y2 - SHAPE_MIN) : from.y),
    w,
    h,
  };
}

/** `v` with tag `digit` on `target`. An element holds one tag, so any other it had goes; with
 * `toggle`, putting a tag on the element it's already on takes it off. */
export function setTag(v: View, digit: string, target: string, toggle = false): View {
  if (toggle && v.tags[digit] === target) {
    const { [digit]: _, ...rest } = v.tags;
    return { ...v, tags: rest };
  }
  const tags = Object.fromEntries(Object.entries(v.tags).filter(([, on]) => on !== target));
  return { ...v, tags: { ...tags, [digit]: target } };
}

/** The tag on `target`, if it has one. */
export const tagOf = (v: View, target: string) =>
  Object.entries(v.tags).find(([, on]) => on === target)?.[0];

/** `v` without the selected shapes, notes, arrows, groups (keeping their members) and edge
 * styles, and without the tags on them. Parts stay. */
export function removeSelected(v: View, sel: Set<string>, ungroupAll: (v: View) => View): View {
  const shapes = new Set(idsOf(sel, "part").filter((id) => id.startsWith("s:")));
  const notes = new Set(idsOf(sel, "note"));
  const arrows = new Set(idsOf(sel, "arrow"));
  const groups = new Set(idsOf(sel, "group"));
  const edges = new Set(idsOf(sel, "edge"));
  const gone = new Set([...shapes, ...notes, ...arrows, ...groups]);
  let out = ungroupAll(shapes.size ? moveInto(v, [...shapes], undefined) : v);
  out = {
    ...out,
    shapes: out.shapes.filter((s) => !shapes.has(s.id)),
    notes: out.notes.filter((n) => !notes.has(n.id)),
    arrows: out.arrows.filter((a) => !arrows.has(a.id)),
    edges: Object.fromEntries(Object.entries(out.edges).filter(([k]) => !edges.has(k))),
    tags: Object.fromEntries(Object.entries(out.tags).filter(([, on]) => !gone.has(on))),
  };
  return out;
}

/** The ids of the boxes whose centers sit inside `r`. */
export function inside(boxes: Map<string, Box>, r: Box): string[] {
  return [...boxes]
    .filter(([, b]) => {
      const cx = b.x + b.w / 2;
      const cy = b.y + b.h / 2;
      return cx > r.x && cx < r.x + r.w && cy > r.y && cy < r.y + r.h;
    })
    .map(([id]) => id);
}
