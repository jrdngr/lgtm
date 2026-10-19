import type { ReactNode } from "react";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { NEUTRAL, tint } from "./colors";
import { Chevron, Fit, Minus, Person, Plus } from "./glyphs";
import { curve, type Layout, type Placed } from "./layout";
import { deltaText } from "./lens";
import { usePanZoom } from "./panZoom";
import type { SEdge } from "./scene";

interface Props {
  layout: Layout;
  edges: SEdge[];
  /** Nodes drawn in full; the rest fade. All of them when absent. */
  lit?: Set<string>;
  /** Edges drawn in `hotColor`, by `from>to`, some numbered. */
  hot: Map<string, number | undefined>;
  hotColor: string;
  /** Labels every described edge, not only hot ones. */
  labels: boolean;
  selected?: string;
  /** The selected edge, as `from>to`. */
  selectedEdge?: string;
  /** Nodes whose description is out of date. */
  stale: Set<string>;
  /** Drawn over the canvas, outside the world. */
  children?: ReactNode;
  onSelect(id: string | undefined): void;
  onOpen(id: string): void;
  onEdge(e: SEdge): void;
  nodeMenu(id: string): CtxEntry[];
  onReload(): void;
}

const EDGE = "#3c3f45";

export const edgeKey = (e: { from: string; to: string }) => `${e.from}>${e.to}`;

/** An edge's stroke: wider with more import sites. */
const width = (e: SEdge) => (e.imports ? Math.min(3.5, 1.4 + Math.log2(e.imports) / 3) : 1.6);

/** A tile's width for a component of `files` files. */
const tileWidth = (files: number) => Math.round(Math.min(40, 8 + Math.sqrt(files) * 5));

const STORE = /sql|database|\bdb\b|store|cache|redis/i;

/** What a review-changed edge's pill says. */
function statusLabel(e: SEdge) {
  if (e.status === "removed") return "removed";
  return `new · ${e.label ?? `${e.imports} ${e.imports === 1 ? "site" : "sites"}`}`;
}

/** A node's content. */
function Node({ n, stale }: { n: Placed; stale: boolean }) {
  const color = n.color ?? NEUTRAL;
  const swatch = n.color && <span className="sys-sw" style={{ background: n.color }} />;
  return (
    <>
      {n.kind === "person" && <Person />}
      <span className="sys-hd">
        {n.kind === "system" && <span className="sys-sw round" style={{ background: NEUTRAL }} />}
        {n.kind !== "component" && !n.door && swatch}
        <span className="sys-nm">{n.name}</span>
        {(n.kind === "external" || (n.kind === "container" && (n.tech?.length ?? 0) <= 10)) &&
          n.tech && <span className="sys-chip">{n.tech}</span>}
        {n.door && <Chevron />}
        {stale && <span className="sys-stale" />}
        {n.kind === "component" && (n.delta || n.status === "added") && (
          <span className={`sys-delta${n.status === "added" ? " new" : ""}`}>
            {n.status === "added" ? "new" : deltaText(n.delta)}
          </span>
        )}
      </span>
      {n.kind === "container" && !n.door && n.delta && (
        <span className="sys-cnt">{deltaText(n.delta)}</span>
      )}
      {n.flag && <span className="sys-flag">!</span>}
      {n.kind === "container" && !n.door && (n.tech?.length ?? 0) > 10 && (
        <span className="sys-chip">{n.tech}</span>
      )}
      {!!n.tiles?.length && (
        <span className="sys-tiles">
          {n.tiles.map((t, i) => (
            <span
              // biome-ignore lint/suspicious/noArrayIndexKey: tiles have no identity
              key={i}
              className={`sys-tile${t.changed ? " c" : ""}`}
              style={{ width: tileWidth(t.files), background: tint(color, 0.6) }}
            />
          ))}
        </span>
      )}
      {!!n.tiles?.length && (
        <span className="sys-go" style={{ background: color }}>
          <Chevron />
        </span>
      )}
      {n.blocks && (
        <span className="sys-blocks">
          {n.blocks.map((b, i) => (
            <span
              key={b.name}
              className="sys-block"
              style={{
                background: tint(b.color, 0.35),
                gridRow: i < 2 && n.blocks && n.blocks.length > 2 ? "span 2" : undefined,
              }}
            >
              {b.name}
            </span>
          ))}
        </span>
      )}
    </>
  );
}

/** The diagram: fitted to the tab, then dragged to pan and scrolled or pinched to zoom. */
export function SystemCanvas(props: Props) {
  const { layout, edges, lit, hot, hotColor, labels, selected, stale, onSelect, onOpen } = props;
  const { selectedEdge, onEdge, nodeMenu } = props;
  const pz = usePanZoom(layout.width, layout.height);
  const { view } = pz;
  const box = new Map(layout.nodes.map((n) => [n.id, n]));
  const lines = edges.flatMap((e) => {
    const a = box.get(e.from);
    const b = box.get(e.to);
    return a && b ? [{ e, door: !!(a.door || b.door), ...curve(a, b) }] : [];
  });
  const markers = { base: EDGE, hot: hotColor, added: "#3fb950", removed: "#f85149" };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel
    <div
      className="sys-canvas"
      ref={pz.ref}
      onPointerDown={pz.onPointerDown}
      onClick={() => {
        if (!pz.dragged()) onSelect(undefined);
      }}
      onContextMenu={(e) =>
        openMenu(e, [
          { label: "Fit to Window", onSelect: pz.fit },
          { label: "Reload", onSelect: props.onReload },
        ])
      }
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
              ...(layout.boundary.color && {
                border: `1.5px solid ${tint(layout.boundary.color, 0.45)}`,
                background: tint(layout.boundary.color, 0.03),
              }),
            }}
          >
            <span
              style={{
                color: layout.boundary.color ? tint(layout.boundary.color, 0.85) : undefined,
              }}
            >
              <span
                className={`sys-sw${layout.boundary.color ? "" : " round"}`}
                style={{ background: layout.boundary.color ?? NEUTRAL }}
              />
              {layout.boundary.name}
            </span>
          </div>
        )}
        {layout.tiers.map((t) => (
          <div
            key={t.label}
            className="sys-band"
            style={{
              left: t.x,
              top: t.y,
              width: t.w,
              height: t.h,
              background: tint(layout.boundary?.color ?? NEUTRAL, 0.04),
            }}
          >
            <span style={{ color: tint(layout.boundary?.color ?? NEUTRAL, 0.7) }}>{t.label}</span>
          </div>
        ))}
        <svg className="sys-edges" width={layout.width} height={layout.height} aria-hidden="true">
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
          {lines.map(({ e, d, door }) => {
            const on = hot.has(edgeKey(e)) || edgeKey(e) === selectedEdge;
            const kind = e.status ?? (on ? "hot" : "base");
            const cls = ["sys-edge", e.status, door && "door", lit && !on && "faint"];
            return (
              <g key={edgeKey(e)}>
                <path
                  className={cls.filter(Boolean).join(" ")}
                  d={d}
                  fill="none"
                  style={on && !e.status ? { stroke: hotColor } : undefined}
                  strokeWidth={on ? width(e) + 0.6 : width(e)}
                  markerEnd={`url(#sys-arrow-${kind})`}
                />
                {!door && (
                  <path
                    className="sys-ehit"
                    d={d}
                    fill="none"
                    onClick={(ev) => {
                      ev.stopPropagation();
                      if (!pz.dragged()) onEdge(e);
                    }}
                  />
                )}
              </g>
            );
          })}
        </svg>
        {lines.map(({ e, mid }) => {
          if (e.status && (!lit || hot.has(edgeKey(e)) || labels)) {
            return (
              <span
                key={edgeKey(e)}
                className={`sys-epill ${e.status}`}
                style={{ left: mid.x, top: mid.y }}
              >
                {statusLabel(e)}
              </span>
            );
          }
          const n = hot.get(edgeKey(e));
          if (n !== undefined) {
            return (
              <span key={edgeKey(e)} className="sys-num" style={{ left: mid.x, top: mid.y }}>
                {n}
              </span>
            );
          }
          return e.label && (labels || hot.has(edgeKey(e))) ? (
            <span
              key={edgeKey(e)}
              className="sys-elabel"
              style={{
                left: mid.x,
                top: mid.y,
                color: hot.has(edgeKey(e)) ? tint(hotColor, 0.85) : undefined,
              }}
            >
              {e.label}
            </span>
          ) : null;
        })}
        {layout.nodes.map((n) => (
          <button
            key={n.id}
            type="button"
            className={nodeClass(n, lit)}
            style={nodeStyle(n, n.id === selected, lit?.has(n.id))}
            onClick={(ev) => {
              ev.stopPropagation();
              if (!pz.dragged()) onSelect(n.id);
            }}
            onDoubleClick={() => onOpen(n.id)}
            onContextMenu={(ev) => {
              ev.stopPropagation();
              const items = nodeMenu(n.id);
              if (!items.length) return;
              if (!n.door) onSelect(n.id);
              openMenu(ev, items);
            }}
          >
            <Node n={n} stale={stale.has(n.id)} />
          </button>
        ))}
      </div>
      {props.children}
      <div className="sys-zoom">
        <button type="button" aria-label="Zoom In" onClick={pz.zoomIn}>
          <Plus />
        </button>
        <button type="button" aria-label="Zoom Out" onClick={pz.zoomOut}>
          <Minus />
        </button>
        <button type="button" aria-label="Fit" onClick={pz.fit}>
          <Fit />
        </button>
      </div>
    </div>
  );
}

function nodeClass(n: Placed, lit: Set<string> | undefined) {
  const store = n.kind === "container" && !n.tiles?.length && !!n.tech && STORE.test(n.tech);
  return [
    "sys-node",
    n.kind,
    n.door && "door",
    store && "store",
    n.status,
    lit && !lit.has(n.id) && "dim",
  ]
    .filter(Boolean)
    .join(" ");
}

function nodeStyle(n: Placed, selected: boolean, lit: boolean | undefined): React.CSSProperties {
  const color = n.color ?? NEUTRAL;
  const style: React.CSSProperties = { left: n.x, top: n.y, width: n.w, height: n.h };
  if (selected) {
    style.border = `1.5px solid ${color}`;
    style.boxShadow = `0 0 0 4px ${tint(color, 0.18)}, 0 8px 20px rgb(0 0 0 / 28%)`;
    if (n.kind === "component") style.background = tint(color, 0.16);
  } else if (lit && n.kind === "component") {
    style.borderColor = tint(color, 0.45);
    style.background = tint(color, 0.07);
  } else if (n.door) {
    style.borderColor = tint(color, 0.6);
    style.background = tint(color, 0.08);
    style.color = color;
  }
  return style;
}
