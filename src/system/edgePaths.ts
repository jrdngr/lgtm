import type { Box } from "./layout";

export interface Pt {
  x: number;
  y: number;
}

const center = (b: Box): Pt => ({ x: b.x + b.w / 2, y: b.y + b.h / 2 });

/** Where the line from `b`'s center toward `p` leaves `b`. */
export function boxExit(b: Box, p: Pt): Pt {
  const c = center(b);
  const dx = p.x - c.x;
  const dy = p.y - c.y;
  const t = Math.min(
    dx ? b.w / 2 / Math.abs(dx) : Number.POSITIVE_INFINITY,
    dy ? b.h / 2 / Math.abs(dy) : Number.POSITIVE_INFINITY,
  );
  return Number.isFinite(t) ? { x: c.x + dx * t, y: c.y + dy * t } : c;
}

/** A wire's shape between two boxes: curved through its middle, or in right angles, with the
 * middle moved by `bend` from halfway between the boxes' centers. */
export interface Shape {
  route: "curved" | "elbow";
  bend?: Pt | null;
}

/** The path of a shaped wire from `a` to `b`, and its middle. */
export function shapedPath(a: Box, b: Box, s: Shape): { d: string; mid: Pt } {
  const ca = center(a);
  const cb = center(b);
  const m = {
    x: (ca.x + cb.x) / 2 + (s.bend?.x ?? 0),
    y: (ca.y + cb.y) / 2 + (s.bend?.y ?? 0),
  };
  if (s.route === "elbow") {
    const across = Math.abs(cb.x - ca.x) >= Math.abs(cb.y - ca.y);
    if (across) {
      const right = cb.x > ca.x;
      const p = { x: right ? a.x + a.w : a.x, y: ca.y };
      const q = { x: right ? b.x : b.x + b.w, y: cb.y };
      return { d: `M${p.x} ${p.y}H${m.x}V${q.y}H${q.x}`, mid: { x: m.x, y: (p.y + q.y) / 2 } };
    }
    const down = cb.y > ca.y;
    const p = { x: ca.x, y: down ? a.y + a.h : a.y };
    const q = { x: cb.x, y: down ? b.y : b.y + b.h };
    return { d: `M${p.x} ${p.y}V${m.y}H${q.x}V${q.y}`, mid: { x: (p.x + q.x) / 2, y: m.y } };
  }
  const p = boxExit(a, m);
  const q = boxExit(b, m);
  if (!s.bend) {
    // Bowed to the right of its direction, as an unshaped wire is.
    const dx = q.x - p.x;
    const dy = q.y - p.y;
    const len = Math.hypot(dx, dy) || 1;
    const bow = Math.min(60, Math.max(12, len * 0.12));
    const c = { x: (p.x + q.x) / 2 - (dy / len) * bow, y: (p.y + q.y) / 2 + (dx / len) * bow };
    return {
      d: `M${p.x} ${p.y}Q${c.x} ${c.y} ${q.x} ${q.y}`,
      mid: { x: (p.x + 2 * c.x + q.x) / 4, y: (p.y + 2 * c.y + q.y) / 4 },
    };
  }
  const c = { x: 2 * m.x - (p.x + q.x) / 2, y: 2 * m.y - (p.y + q.y) / 2 };
  return { d: `M${p.x} ${p.y}Q${c.x} ${c.y} ${q.x} ${q.y}`, mid: m };
}

/** A gently bowed path from `p` to `q`, for a drawn arrow. */
export function arrowPath(p: Pt, q: Pt): string {
  const dx = q.x - p.x;
  const dy = q.y - p.y;
  const len = Math.hypot(dx, dy) || 1;
  const bow = Math.min(40, len * 0.15);
  const c = { x: (p.x + q.x) / 2 - (dy / len) * bow, y: (p.y + q.y) / 2 + (dx / len) * bow };
  return `M${p.x} ${p.y}Q${c.x} ${c.y} ${q.x} ${q.y}`;
}
