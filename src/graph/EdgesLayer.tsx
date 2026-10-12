import { ViewportPortal } from "@xyflow/react";
import { useMemo } from "react";
import { NODE_HEIGHT, type Placed } from "./layout";
import type { Link } from "./view";

interface Paths {
  links: string;
  callers: string;
  arrows: string;
}

const empty = (): Paths => ({ links: "", callers: "", arrows: "" });

/** Every link as one path per style. An SVG element per link costs WebKit time that grows
 * faster than the number of links: 250 links blocked the window for 2 s. */
function paths(links: Link[], placed: Placed, only?: string): Paths {
  const out = empty();
  for (const l of links) {
    if (only !== undefined && l.from !== only && l.to !== only) continue;
    const a = placed.boxes.get(l.from)?.abs;
    const b = placed.boxes.get(l.to)?.abs;
    if (!a || !b) continue;
    const [sx, sy] = [a.x + a.width, a.y + Math.min(a.height, NODE_HEIGHT) / 2];
    const [tx, ty] = [b.x, b.y + Math.min(b.height, NODE_HEIGHT) / 2];
    const dx = Math.max(40, Math.abs(tx - sx) / 2);
    const d = `M${sx},${sy}C${sx + dx},${sy} ${tx - dx},${ty} ${tx},${ty}`;
    if (l.caller) out.callers += d;
    else out.links += d;
    out.arrows += `M${tx},${ty}l-7,-4v8z`;
  }
  return out;
}

/** The graph's links, drawn under the boxes. Links of `focus` are drawn again on top, so
 * hovering a box shows what it uses and what uses it. */
export function EdgesLayer({
  links,
  placed,
  focus,
}: {
  links: Link[];
  placed: Placed;
  focus?: string;
}) {
  const all = useMemo(() => paths(links, placed), [links, placed]);
  const lit = useMemo(
    () => (focus === undefined ? empty() : paths(links, placed, focus)),
    [links, placed, focus],
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
