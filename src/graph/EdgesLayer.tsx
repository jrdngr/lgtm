import { ViewportPortal } from "@xyflow/react";
import { useMemo } from "react";
import { NODE_HEIGHT, type Placed, type Point } from "./layout";
import type { Routed } from "./ports";

interface Paths {
  links: string;
  callers: string;
  arrows: string;
  joins: string;
}

const empty = (): Paths => ({ links: "", callers: "", arrows: "", joins: "" });

type Segment = { from: string; to: string; caller: boolean };

/** Where a segment leaves `id` or comes into it: a port's center, or a box's right or left
 * side level with its name. */
function end(placed: Placed, id: string, leaving: boolean) {
  const port = placed.ports.get(id);
  if (port) return port;
  const b = placed.boxes.get(id)?.abs;
  if (!b) return undefined;
  return { x: leaving ? b.x + b.width : b.x, y: b.y + Math.min(b.height, NODE_HEIGHT) / 2 };
}

/** An arrowhead at `b`, pointing the way the line from `a` goes. */
function arrow(a: Point, b: Point) {
  if (Math.abs(b.x - a.x) >= Math.abs(b.y - a.y)) {
    return b.x >= a.x ? `M${b.x},${b.y}l-7,-4v8z` : `M${b.x},${b.y}l7,-4v8z`;
  }
  return b.y >= a.y ? `M${b.x},${b.y}l-4,-7h8z` : `M${b.x},${b.y}l-4,7h8z`;
}

/** The laid-out right-angled path of a segment, or a curve between its ends. */
function line(placed: Placed, s: Segment): Point[] | undefined {
  const routed = placed.routes.get(`${s.from}>${s.to}`);
  if (routed) return routed.points;
  const a = end(placed, s.from, true);
  const b = end(placed, s.to, false);
  return a && b ? [a, b] : undefined;
}

/** Segments as one path per style. An SVG element per link costs WebKit time that grows
 * faster than the number of links: 250 links blocked the window for 2 s. */
function paths(segments: Iterable<Segment>, placed: Placed, routed: Routed): Paths {
  const out = empty();
  for (const s of segments) {
    const points = line(placed, s);
    if (!points) continue;
    const d = `M${points.map((p) => `${p.x},${p.y}`).join("L")}`;
    if (s.caller) out.callers += d;
    else out.links += d;
    // Arrowheads where a link reaches a box or goes into one, not where it leaves one.
    if (routed.ports.get(s.to)?.side !== "out") {
      out.arrows += arrow(points[points.length - 2], points[points.length - 1]);
    }
    for (const j of placed.routes.get(`${s.from}>${s.to}`)?.joins ?? []) {
      out.joins += `M${j.x - 2.5},${j.y}a2.5,2.5 0 1,0 5,0a2.5,2.5 0 1,0 -5,0`;
    }
  }
  return out;
}

/** The segments of every link that starts or ends at `box`, or passes through its ports. */
function litSegments(routed: Routed, box: string): Segment[] {
  const out = new Map<string, Segment>();
  for (const r of routed.routes) {
    const touches =
      r.link.from === box ||
      r.link.to === box ||
      r.stops.some((s) => routed.ports.get(s)?.box === box);
    if (!touches) continue;
    for (let i = 1; i < r.stops.length; i++) {
      const key = `${r.stops[i - 1]}>${r.stops[i]}`;
      const s = routed.segments.get(key);
      if (s) out.set(key, s);
    }
  }
  return [...out.values()];
}

/** The graph's links and ports, drawn under the boxes. Links of `focus` are drawn again on
 * top, so hovering a box shows what it uses and what uses it. */
export function EdgesLayer({
  routed,
  placed,
  focus,
}: {
  routed: Routed;
  placed: Placed;
  focus?: string;
}) {
  const all = useMemo(() => paths(routed.segments.values(), placed, routed), [routed, placed]);
  const ports = useMemo(
    () => [...placed.ports.values()].map((p) => `M${p.x - 3},${p.y - 3}h6v6h-6z`).join(""),
    [placed],
  );
  const lit = useMemo(
    () => (focus === undefined ? empty() : paths(litSegments(routed, focus), placed, routed)),
    [routed, placed, focus],
  );
  return (
    <ViewportPortal>
      <svg className={`glinks${focus === undefined ? "" : " dimmed"}`} aria-hidden="true">
        <path className="glink" d={all.links} />
        <path className="glink caller" d={all.callers} />
        <path className="garrow" d={all.arrows + all.joins} />
        <path className="gport" d={ports} />
        <path className="glink lit" d={lit.links + lit.callers} />
        <path className="garrow lit" d={lit.arrows + lit.joins} />
      </svg>
    </ViewportPortal>
  );
}
