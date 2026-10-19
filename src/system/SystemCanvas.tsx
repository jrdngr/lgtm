import { openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { curve, type Layout } from "./layout";
import { usePanZoom } from "./panZoom";
import "../graph/graph.css";
import type { SEdge } from "./scene";

interface Props {
  layout: Layout;
  /** The edges to draw. */
  edges: SEdge[];
  /** Labels only the selection's edges. */
  quiet: boolean;
  selected?: string;
  onSelect(id: string | undefined): void;
  onOpen(id: string): void;
}

/** An edge's stroke: wider with more import sites. */
const width = (e: SEdge) => (e.imports ? Math.min(4, 1 + Math.log2(e.imports) / 2) : 1.4);

/** The diagram: fitted to the tab, then dragged to pan and scrolled or pinched to zoom. */
export function SystemCanvas({ layout, edges, quiet, selected, onSelect, onOpen }: Props) {
  const pz = usePanZoom(layout.width, layout.height);
  const { view } = pz;

  const box = new Map(layout.nodes.map((n) => [n.id, n]));
  const lines = edges.flatMap((e) => {
    const a = box.get(e.from);
    const b = box.get(e.to);
    return a && b ? [{ e, ...curve(a, b) }] : [];
  });
  const touches = (e: SEdge) => e.from === selected || e.to === selected;

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel
    <div
      className="sys-canvas"
      ref={pz.ref}
      onPointerDown={pz.onPointerDown}
      onClick={() => {
        if (!pz.dragged()) onSelect(undefined);
      }}
      onContextMenu={(e) => openMenu(e, [{ label: "Fit to Window", onSelect: pz.fit }])}
    >
      <div
        className="sys-world"
        style={{
          width: layout.width,
          height: layout.height,
          transform: `translate(${view.x}px, ${view.y}px) scale(${view.k})`,
        }}
      >
        {layout.boundary && (
          <div
            className="sys-boundary"
            style={{
              left: layout.boundary.x,
              top: layout.boundary.y,
              width: layout.boundary.w,
              height: layout.boundary.h,
            }}
          >
            <span>{layout.boundary.name}</span>
          </div>
        )}
        {layout.tiers.map((t) => (
          <div key={t.label} className="sys-tier" style={{ left: t.x, top: t.y }}>
            {t.label}
          </div>
        ))}
        <svg className="sys-edges" width={layout.width} height={layout.height} aria-hidden="true">
          <defs>
            {["same", "on", "added", "removed"].map((k) => (
              <marker
                key={k}
                id={`sys-arrow-${k}`}
                className={`sys-arrow ${k}`}
                viewBox="0 0 8 8"
                refX="7"
                refY="4"
                markerWidth="8"
                markerHeight="8"
                markerUnits="userSpaceOnUse"
                orient="auto"
              >
                <path d="M0 0L8 4L0 8z" />
              </marker>
            ))}
          </defs>
          {lines.map(({ e, d }) => {
            const kind = e.status ?? (touches(e) ? "on" : "same");
            return (
              <path
                key={`${e.from}>${e.to}`}
                className={`sys-edge ${kind}`}
                d={d}
                fill="none"
                strokeWidth={width(e)}
                markerEnd={`url(#sys-arrow-${kind})`}
              />
            );
          })}
        </svg>
        {lines.map(({ e, mid }) => {
          const text = e.label ?? (e.imports || undefined);
          return text === undefined || (quiet && !touches(e)) ? null : (
            <div
              key={`${e.from}>${e.to}`}
              className={`sys-elabel${e.status ? ` ${e.status}` : ""}${touches(e) ? " on" : ""}`}
              style={{ left: mid.x, top: mid.y }}
            >
              {text}
            </div>
          );
        })}
        {layout.nodes.map((n) => (
          <button
            key={n.id}
            type="button"
            className={`sys-node ${n.kind}${n.status ? ` ${n.status}` : ""}${n.id === selected ? " sel" : ""}`}
            style={{ left: n.x, top: n.y, width: n.w, height: n.h }}
            onClick={(ev) => {
              ev.stopPropagation();
              onSelect(n.id);
            }}
            onDoubleClick={() => onOpen(n.id)}
          >
            {n.kind !== "component" && <span className="sys-kind">{n.label}</span>}
            <span className="sys-name">{n.name}</span>
            {n.description && <span className="sys-desc">{n.description}</span>}
            {n.meta && <span className="sys-meta">{n.meta}</span>}
          </button>
        ))}
      </div>
      <div className="gzoom sys-zoom">
        <IconButton icon="zoomIn" label="Zoom in" onClick={pz.zoomIn} />
        <IconButton icon="zoomOut" label="Zoom out" onClick={pz.zoomOut} />
        <IconButton icon="fit" label="Fit" onClick={pz.fit} />
      </div>
    </div>
  );
}
