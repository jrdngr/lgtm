import type { Point, Rect } from "../graph/layout";
import type { Focus, SceneEdge } from "./scene";

/** A polyline with its corners rounded. */
export function roundedPath(points: Point[], r = 6): string {
  if (points.length === 0) return "";
  let d = `M${points[0].x} ${points[0].y}`;
  for (let i = 1; i < points.length - 1; i++) {
    const [a, b, c] = [points[i - 1], points[i], points[i + 1]];
    const k = Math.min(r, dist(a, b) / 2, dist(b, c) / 2);
    const p = toward(b, a, k);
    const q = toward(b, c, k);
    d += `L${p.x} ${p.y}Q${b.x} ${b.y} ${q.x} ${q.y}`;
  }
  const last = points[points.length - 1];
  return `${d}L${last.x} ${last.y}`;
}

const dist = (a: Point, b: Point) => Math.hypot(b.x - a.x, b.y - a.y);

function toward(from: Point, to: Point, by: number): Point {
  const d = dist(from, to) || 1;
  return { x: from.x + ((to.x - from.x) * by) / d, y: from.y + ((to.y - from.y) * by) / d };
}

/** A filled arrowhead with its tip at `b`, pointing the way the line from `a` goes. */
export function arrowHead(a: Point, b: Point): string {
  const back = toward(b, a, 8);
  const nx = (b.y - a.y) / (dist(a, b) || 1);
  const ny = -(b.x - a.x) / (dist(a, b) || 1);
  return `M${b.x} ${b.y}L${back.x + nx * 4} ${back.y + ny * 4}L${back.x - nx * 4} ${back.y - ny * 4}z`;
}

/** Halfway along a polyline. */
export function midpoint(points: Point[]): Point {
  const lengths = points.slice(1).map((p, i) => dist(points[i], p));
  let left = lengths.reduce((s, l) => s + l, 0) / 2;
  for (let i = 0; i < lengths.length; i++) {
    if (left <= lengths[i]) return toward(points[i], points[i + 1], left);
    left -= lengths[i];
  }
  return points[points.length - 1] ?? { x: 0, y: 0 };
}

/** One path per class: an SVG element per edge costs WebKit time that grows faster than the
 * number of edges. */
export interface StyledPaths {
  /** `edge <style>` plus `dim` or `hi` under a focus, to its lines and its arrowheads. */
  lines: Map<string, { d: string; arrows: string }>;
  /** Where to draw the ✕ of each edge that breaks a Wrong mark. */
  badges: Point[];
}

/** Groups the edges' paths by how they draw. Under a focus, edges outside it dim and edges in
 * it stand out. */
export function styledPaths(
  edges: SceneEdge[],
  routes: Map<string, Point[]>,
  focus?: Focus,
): StyledPaths {
  const lines: StyledPaths["lines"] = new Map();
  const badges: Point[] = [];
  for (const e of edges) {
    const points = routes.get(e.key);
    if (!points || points.length < 2) continue;
    const emphasis = !focus ? "" : focus.edges.has(e.key) ? " hi" : " dim";
    const cls = `edge ${e.style}${emphasis}`;
    const at = lines.get(cls) ?? { d: "", arrows: "" };
    at.d += roundedPath(points);
    at.arrows += arrowHead(points[points.length - 2], points[points.length - 1]);
    lines.set(cls, at);
    if (e.style === "wrong") badges.push(midpoint(points));
  }
  return { lines, badges };
}

/** How far `p` is from the segment `a`–`b`. */
function segmentDistance(p: Point, a: Point, b: Point): number {
  const len2 = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
  const t = len2 === 0 ? 0 : ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / len2;
  const c = Math.max(0, Math.min(1, t));
  return dist(p, { x: a.x + c * (b.x - a.x), y: a.y + c * (b.y - a.y) });
}

/** The key of the edge nearest `p`, within `within`. Edges share one path per style, so
 * clicks find theirs by distance. */
export function edgeAt(
  edges: SceneEdge[],
  routes: Map<string, Point[]>,
  p: Point,
  within: number,
): string | undefined {
  let best: { key: string; d: number } | undefined;
  for (const e of edges) {
    const points = routes.get(e.key) ?? [];
    for (let i = 1; i < points.length; i++) {
      const d = segmentDistance(p, points[i - 1], points[i]);
      if (d <= within && (!best || d < best.d)) best = { key: e.key, d };
    }
  }
  return best?.key;
}

/** A box around `rects`, `pad` out on each side, with room above for a label. */
export function around(rects: Rect[], pad: number, label: number): Rect | undefined {
  if (rects.length === 0) return undefined;
  const x0 = Math.min(...rects.map((r) => r.x)) - pad;
  const y0 = Math.min(...rects.map((r) => r.y)) - pad - label;
  const x1 = Math.max(...rects.map((r) => r.x + r.width)) + pad;
  const y1 = Math.max(...rects.map((r) => r.y + r.height)) + pad;
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}
