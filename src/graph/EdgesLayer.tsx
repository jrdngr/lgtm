import { ViewportPortal } from "@xyflow/react";
import { useMemo } from "react";
import type { GraphDto } from "../bindings";
import { NODE_HEIGHT, type Placed } from "./layout";

interface Paths {
  links: string;
  callers: string;
  arrows: string;
}

const empty = (): Paths => ({ links: "", callers: "", arrows: "" });

/** Every link as one path per style. An SVG element per link costs WebKit time that grows
 * faster than the number of links: 250 links blocked the window for 2 s. */
function paths(graph: GraphDto, placed: Placed, only?: number): Paths {
  const at = (id: number) => {
    const box = placed.nodes.get(id);
    const group = box && placed.groups.get(graph.nodes[id]?.path ?? "");
    return box && group ? { x: group.x + box.x, y: group.y + box.y, w: box.width } : undefined;
  };
  const out = empty();
  for (const e of graph.edges) {
    if (only !== undefined && e.from !== only && e.to !== only) continue;
    const a = at(e.from);
    const b = at(e.to);
    if (!a || !b) continue;
    const [sx, sy] = [a.x + a.w, a.y + NODE_HEIGHT / 2];
    const [tx, ty] = [b.x, b.y + NODE_HEIGHT / 2];
    const dx = Math.max(40, Math.abs(tx - sx) / 2);
    const d = `M${sx},${sy}C${sx + dx},${sy} ${tx - dx},${ty} ${tx},${ty}`;
    if (graph.nodes[e.from]?.status === "caller") out.callers += d;
    else out.links += d;
    out.arrows += `M${tx},${ty}l-7,-4v8z`;
  }
  return out;
}

/** The graph's links, drawn under the nodes. Links of `focus` are drawn again on top, so
 * hovering a symbol shows what it uses and what uses it. */
export function EdgesLayer({
  graph,
  placed,
  focus,
}: {
  graph: GraphDto;
  placed: Placed;
  focus?: number;
}) {
  const all = useMemo(() => paths(graph, placed), [graph, placed]);
  const lit = useMemo(
    () => (focus === undefined ? empty() : paths(graph, placed, focus)),
    [graph, placed, focus],
  );
  return (
    <ViewportPortal>
      <svg className={`glinks${focus === undefined ? "" : " dimmed"}`} aria-hidden="true">
        <path className="glink" d={all.links} />
        <path className="glink caller" d={all.callers} />
        <path className="garrow" d={all.arrows} />
        <path className="glink lit" d={lit.links + lit.callers} />
        <path className="garrow lit" d={lit.arrows} />
      </svg>
    </ViewportPortal>
  );
}
