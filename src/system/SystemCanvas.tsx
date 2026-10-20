import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { EdgeLabels } from "../bindings";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { NEUTRAL, tint } from "./colors";
import { Fit, Minus, Plus } from "./glyphs";
import type { Layout, Placed } from "./layout";
import { EdgePills, MapEdges } from "./MapEdges";
import { edgeKey, edgesOf, focusWires, mapFocus, restWires } from "./mapWires";
import { usePanZoom } from "./panZoom";
import { Node, nodeClass, nodeStyle, type Spot } from "./SystemNode";
import type { SEdge } from "./scene";
import { useAnimating } from "./useAnimating";
import { neighborhood } from "./view";

interface Props {
  layout: Layout;
  edges: SEdge[];
  /** Nodes drawn in full at rest; the rest fade. All of them when absent. */
  lit?: Set<string>;
  labels: EdgeLabels;
  /** Edges drawn bright in `hotColor` at rest, by `from>to`, some numbered. */
  hot: Map<string, number | undefined>;
  hotColor: string;
  selected?: string;
  /** The selected edge, as `from>to`. */
  selectedEdge?: string;
  /** The node the canvas collapses around. */
  focus?: string;
  /** Nodes whose description is out of date. */
  stale: Set<string>;
  /** Drawn over the canvas, outside the world. */
  children?: ReactNode;
  onSelect(id: string | undefined): void;
  /** Opens a node a level down: ⌘-click, double-click, or a click on the focused node. */
  onOpen(id: string): void;
  onEdge(e: SEdge): void;
  nodeMenu(id: string): CtxEntry[];
  onReload(): void;
  /** Changed edges show no status pills, only their labels when lit. */
  quiet?: boolean;
}

type Hover = { node: string } | { edge: string };

/** The diagram, fitted to the tab, then dragged to pan and scrolled or pinched to zoom. At rest
 * every edge is a hairline; hovering a node or edge draws it bright. Focusing a node collapses
 * the canvas into it, the nodes that use it and the ones it uses. */
export function SystemCanvas(props: Props) {
  const { layout, edges, hot, hotColor, selected, selectedEdge, stale, onSelect, onOpen } = props;
  const [hover, setHover] = useState<Hover>();
  const cmd = useCmdHeld();
  const focus = useMemo(
    () => (props.focus ? mapFocus(edges, props.focus) : undefined),
    [edges, props.focus],
  );
  const pz = usePanZoom(focus?.width ?? layout.width, focus?.height ?? layout.height);
  const { view } = pz;
  const anim = useAnimating(props.focus);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new focus or level drops the hover
  useEffect(() => setHover(undefined), [props.focus, layout]);

  const hoverNode = hover && "node" in hover ? hover.node : undefined;
  const hoverEdge = hover && "edge" in hover ? hover.edge : undefined;
  const hovered = new Set<string>();
  let lit = props.lit;
  if (hover && !focus) {
    if ("edge" in hover) {
      hovered.add(hover.edge);
      lit = new Set(hover.edge.split(">"));
    } else {
      const mine = edgesOf(edges, hover.node);
      for (const e of mine) hovered.add(edgeKey(e));
      lit = neighborhood(mine, hover.node);
    }
  }
  const doors = new Set(layout.nodes.filter((n) => n.door).map((n) => n.id));
  const wires =
    focus && props.focus
      ? focusWires(
          focus,
          props.focus,
          edges,
          doors,
          { node: hoverNode, edge: hoverEdge },
          selectedEdge,
          props.quiet,
        )
      : restWires(layout, edges, { hovered, hot, selectedEdge, quiet: props.quiet });
  const spot = (n: Placed): Spot => {
    if (!focus) return { box: n, compact: false };
    const at = focus.pos.get(n.id);
    return at && { box: at, compact: true };
  };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel
    <div
      className={`sys-canvas${focus ? " sys-focus" : ""}${cmd ? " cmd" : ""}`}
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
        className={`sys-world${anim ? " anim" : ""}`}
        style={{
          width: layout.width,
          height: layout.height,
          transform: `translate(${view.x}px, ${view.y}px) scale(${view.k})`,
        }}
      >
        <div className="sys-sctx">
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
        </div>
        {/* Keyed by the focus so a collapsed graph's wires fade in once the nodes have moved. */}
        <div key={props.focus ?? "rest"} className={focus ? "sys-flinks" : undefined}>
          <MapEdges
            wires={wires}
            width={Math.max(layout.width, focus?.width ?? 0)}
            height={Math.max(layout.height, focus?.height ?? 0)}
            hotColor={hotColor}
            hairlines={!focus}
            hovering={hovered.size > 0}
            onHover={(edge) => setHover(edge ? { edge } : undefined)}
            onEdge={(w) => {
              if (!pz.dragged()) props.onEdge(w.e);
            }}
          />
          <EdgePills
            wires={wires}
            labels={props.labels}
            selectedEdge={selectedEdge}
            hotColor={hotColor}
            onHover={focus ? (edge) => setHover(edge ? { edge } : undefined) : undefined}
          />
        </div>
        {layout.nodes.map((n) => {
          const at = spot(n);
          return (
            <button
              key={n.id}
              type="button"
              className={nodeClass(n, at, focus ? undefined : lit)}
              style={nodeStyle(n, at, n.id === selected, !focus && !!lit?.has(n.id))}
              onPointerEnter={() => setHover({ node: n.id })}
              onPointerLeave={() =>
                setHover((h) => (h && "node" in h && h.node === n.id ? undefined : h))
              }
              onClick={(ev) => {
                ev.stopPropagation();
                if (pz.dragged()) return;
                if (ev.metaKey || ev.ctrlKey || (focus && n.id === props.focus)) onOpen(n.id);
                else onSelect(n.id);
              }}
              onDoubleClick={() => onOpen(n.id)}
              onContextMenu={(ev) => {
                ev.stopPropagation();
                const items = props.nodeMenu(n.id);
                if (!items.length) return;
                if (!n.door) onSelect(n.id);
                openMenu(ev, items);
              }}
            >
              <Node n={n} stale={stale.has(n.id)} />
            </button>
          );
        })}
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

/** Whether ⌘ is held, so the node under the pointer reads as a link. */
function useCmdHeld() {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    const on = (e: KeyboardEvent) => setHeld(e.metaKey);
    const off = () => setHeld(false);
    window.addEventListener("keydown", on);
    window.addEventListener("keyup", on);
    window.addEventListener("blur", off);
    return () => {
      window.removeEventListener("keydown", on);
      window.removeEventListener("keyup", on);
      window.removeEventListener("blur", off);
    };
  }, []);
  return held;
}
