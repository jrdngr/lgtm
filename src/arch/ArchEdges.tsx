import { ViewportPortal } from "@xyflow/react";
import { useMemo } from "react";
import type { Rect } from "../graph/layout";
import type { Placed } from "./layout";
import { around, styledPaths } from "./paths";
import type { Focus, SceneEdge, SceneGroup } from "./scene";

interface Props {
  placed: Placed;
  groups: SceneGroup[];
  edges: SceneEdge[];
  boundaries: { name: string; components: string[] }[];
  focus?: Focus;
}

/** Containers, trust boundaries and edges, under the boxes. Edges are one path per style. */
export function ArchEdges({ placed, groups, edges, boundaries, focus }: Props) {
  const paths = useMemo(
    () => styledPaths(edges, placed.routes, focus),
    [edges, placed.routes, focus],
  );
  const trust = boundaries.flatMap((b) => {
    const rects = b.components.flatMap((id) => placed.nodes.get(id) ?? []);
    const r = around(rects, 8, b.name ? 14 : 0);
    return r ? [{ name: b.name, r }] : [];
  });
  return (
    <ViewportPortal>
      <svg className="aedges" aria-hidden="true">
        {groups.map((g) => {
          const r = placed.groups.get(g.id);
          return r && <Group key={g.id} r={r} label={g.label} />;
        })}
        {trust.map(({ name, r }) => (
          <g key={`${name}${r.x},${r.y}`} className="atrust">
            <rect x={r.x} y={r.y} width={r.width} height={r.height} rx={8} />
            {name && (
              <text x={r.x + 6} y={r.y + 13}>
                {name}
              </text>
            )}
          </g>
        ))}
        {[...paths.lines].map(([cls, p]) => (
          <g key={cls} className={cls}>
            <path className="aline-path" d={p.d} />
            <path className="ahead" d={p.arrows} />
          </g>
        ))}
        {placed.joins.map((j) => (
          <circle key={`${j.x},${j.y}`} className="ajoin" cx={j.x} cy={j.y} r={2.5} />
        ))}
        {paths.badges.map((b) => (
          <g key={`${b.x},${b.y}`} className="abadge">
            <circle cx={b.x} cy={b.y} r={8} />
            <text x={b.x} y={b.y + 3}>
              ✕
            </text>
          </g>
        ))}
      </svg>
    </ViewportPortal>
  );
}

function Group({ r, label }: { r: Rect; label: string }) {
  return (
    <g className="agroup">
      <rect className="box" x={r.x} y={r.y} width={r.width} height={r.height} rx={6} />
      <rect className="band" x={r.x + 1} y={r.y + 1} width={r.width - 2} height={22} rx={5} />
      <text x={r.x + 10} y={r.y + 15}>
        {label}
      </text>
    </g>
  );
}
