import { tint } from "./colors";
import type { Tone, Wire } from "./mapWires";

export const BLUE = "#5b9bd5";

interface Props {
  wires: Wire[];
  width: number;
  height: number;
  hotColor: string;
  /** Draws every wire's hairline under the bright ones; faded while `hovering`. */
  hairlines: boolean;
  hovering: boolean;
  onHover(key: string | undefined): void;
  onEdge(w: Wire): void;
}

/** The map's wires in an SVG layer, each with a wide hit path unless it joins a door. */
export function MapEdges(p: Props) {
  const markers: Record<Tone, string> = {
    blue: BLUE,
    hot: p.hotColor,
    added: "#3fb950",
    removed: "#f85149",
  };
  return (
    <svg className="sys-edges" width={p.width} height={p.height} aria-hidden="true">
      <defs>
        {Object.entries(markers).map(([k, fill]) => (
          <marker
            key={k}
            id={`sys-arrow-${k}`}
            viewBox="0 0 8 8"
            refX="7"
            refY="4"
            markerWidth="8"
            markerHeight="8"
            markerUnits="userSpaceOnUse"
            orient="auto"
          >
            <path d="M0 0L8 4L0 8z" fill={fill} />
          </marker>
        ))}
      </defs>
      {p.hairlines && (
        <g className={`sys-hair${p.hovering ? " off" : ""}`}>
          {p.wires.map((w) => (
            <path key={w.key} d={w.d} />
          ))}
        </g>
      )}
      {p.wires.map((w) => {
        if (!w.tone) return null;
        const cls = ["sys-edge", w.e.status, w.door && "door", w.faint && "faint"];
        return (
          <path
            key={w.key}
            className={cls.filter(Boolean).join(" ")}
            d={w.d}
            fill="none"
            style={w.tone === "blue" || w.tone === "hot" ? { stroke: markers[w.tone] } : undefined}
            strokeWidth={w.wide ? 2.6 : 2}
            markerEnd={`url(#sys-arrow-${w.tone})`}
          />
        );
      })}
      {p.wires.map(
        (w) =>
          !w.door && (
            <path
              key={w.key}
              className="sys-ehit"
              d={w.d}
              fill="none"
              onPointerEnter={() => p.onHover(w.key)}
              onPointerLeave={() => p.onHover(undefined)}
              onClick={(ev) => {
                ev.stopPropagation();
                p.onEdge(w);
              }}
            />
          ),
      )}
    </svg>
  );
}

/** The wires' numbers, status pills and labels, in the world over the SVG layer. */
export function EdgePills({
  wires,
  hotColor,
  onHover,
}: {
  wires: Wire[];
  hotColor: string;
  /** In a collapsed graph, labels take hovers like their wires. */
  onHover?(key: string | undefined): void;
}) {
  return wires.map(({ key, pill, mid, faint, anchor, alone }) => {
    if (!pill) return null;
    const at = { left: mid.x, top: mid.y };
    const dim = `${faint ? " faint" : ""}${anchor ? ` at-${anchor}` : ""}${alone ? " alone" : ""}${onHover ? " live" : ""}`;
    const hover = onHover && {
      onPointerEnter: () => onHover(key),
      onPointerLeave: () => onHover(undefined),
    };
    if (pill.kind === "num") {
      return (
        <span key={key} className="sys-num" style={at}>
          {pill.text}
        </span>
      );
    }
    if (pill.kind === "label") {
      const c = pill.hot ? hotColor : BLUE;
      return (
        <span
          key={key}
          className={`sys-elabel${dim}`}
          style={{ ...at, color: tint(c, 0.9), borderColor: tint(c, 0.6) }}
          {...hover}
        >
          {pill.text}
        </span>
      );
    }
    return (
      <span key={key} className={`sys-epill ${pill.kind}${dim}`} style={at} {...hover}>
        {pill.text}
      </span>
    );
  });
}
