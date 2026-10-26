import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { EdgeLabels } from "../bindings";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { NEUTRAL, tint } from "./colors";
import type { Pt } from "./edgePaths";
import { Fit, Minus, Plus } from "./glyphs";
import type { Layout, Placed } from "./layout";
import { EdgePills, MapEdges } from "./MapEdges";
import {
  edgeKey,
  edgesOf,
  focusWires,
  mapFocus,
  type Paint,
  restWires,
  type Wire,
} from "./mapWires";
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
  /** Styled edges, by key. */
  paints?: Map<string, Paint>;
  /** Parts to flash. */
  flash?: Set<string>;
  /** Refits the view when this changes, instead of when the world's size does. */
  fitKey?: string;
  /** Drawn in the world under the edges, and over the nodes. */
  under?: ReactNode;
  over?: ReactNode;
  /** Drawn over the canvas at viewport points. */
  screen?(toScreen: (p: Pt) => Pt, toWorld: ToWorld): ReactNode;
  /** Editing: presses go to these instead of selecting, focusing and panning. */
  edit?: Edit;
}

/** Turns a pointer's client point into a world point. */
export type ToWorld = (clientX: number, clientY: number) => Pt;

export interface Edit {
  selected: Set<string>;
  /** Parts placed since the user arranged the view. */
  fresh: Set<string>;
  /** While held, a drag on the background pans. */
  panning: boolean;
  onNodeDown(id: string, e: React.PointerEvent, toWorld: ToWorld): void;
  onBackgroundDown(e: React.PointerEvent, toWorld: ToWorld): void;
}

type Hover = { node: string } | { edge: string };

/** The diagram, fitted to the tab, then dragged to pan and scrolled or pinched to zoom. At rest
 * every edge is a hairline; hovering a node or edge draws it bright. Focusing a node collapses
 * the canvas into it, the nodes that use it and the ones it uses. */
export function SystemCanvas(props: Props) {
  const { layout, edges, hot, hotColor, selected, selectedEdge, stale, onSelect, onOpen } = props;
  const [hover, setHover] = useState<Hover>();
  const focus = useMemo(
    () => (props.focus ? mapFocus(edges, props.focus) : undefined),
    [edges, props.focus],
  );
  const pz = usePanZoom(
    focus?.width ?? layout.width,
    focus?.height ?? layout.height,
    props.fitKey && `${props.fitKey}${props.focus ?? ""}`,
    focus ? 0 : layout.left,
    focus ? 0 : layout.top,
  );
  const { view } = pz;
  const toWorld: ToWorld = (cx, cy) => {
    const r = pz.ref.current?.getBoundingClientRect();
    return {
      x: (cx - (r?.left ?? 0) - view.x) / view.k,
      y: (cy - (r?.top ?? 0) - view.y) / view.k,
    };
  };
  const toScreen = (p: Pt): Pt => ({ x: view.x + p.x * view.k, y: view.y + p.y * view.k });
  const { edit } = props;
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
      : restWires(layout, edges, {
          hovered,
          hot,
          selectedEdge,
          quiet: props.quiet,
          paints: props.paints,
        });
  const edgeProps = {
    wires,
    width: Math.max(layout.width, focus?.width ?? 0),
    height: Math.max(layout.height, focus?.height ?? 0),
    hotColor,
    hairlines: !focus,
    hovering: hovered.size > 0,
    onHover: (edge: string | undefined) => setHover(edge ? { edge } : undefined),
    onEdge: (w: Wire) => {
      if (!pz.dragged()) props.onEdge(w.e);
    },
  };
  const spot = (n: Placed): Spot => {
    if (!focus) return { box: n, compact: false };
    const at = focus.pos.get(n.id);
    return at && { box: at, compact: true };
  };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel
    <div
      className={`sys-canvas${focus ? " sys-focus" : ""}${edit ? ` cv-edit${edit.panning ? " cv-pan" : ""}` : ""}`}
      ref={pz.ref}
      onPointerDown={(e) => {
        const onNode = (e.target as HTMLElement).closest("button, .cv-hit");
        if (edit && !edit.panning && e.button === 0 && !onNode) {
          edit.onBackgroundDown(e, toWorld);
          return;
        }
        pz.onPointerDown(e);
      }}
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
        {props.under}
        {/* Keyed by the focus so a collapsed graph's wires fade in once the nodes have moved. */}
        <div key={props.focus ?? "rest"} className={focus ? "sys-flinks" : undefined}>
          <MapEdges layer="under" {...edgeProps} />
        </div>
        {layout.nodes.map((n) => {
          const at = spot(n);
          return (
            <button
              key={n.id}
              type="button"
              className={`${nodeClass(n, at, focus ? undefined : lit)}${props.flash?.has(n.id) ? " cv-flash" : ""}`}
              style={nodeStyle(
                n,
                at,
                n.id === selected || !!edit?.selected.has(n.id),
                !focus && !!lit?.has(n.id),
              )}
              onPointerDown={
                edit &&
                ((ev) => {
                  if (ev.button !== 0 || edit.panning) return;
                  ev.stopPropagation();
                  edit.onNodeDown(n.id, ev, toWorld);
                })
              }
              onPointerEnter={() => setHover({ node: n.id })}
              onPointerLeave={() =>
                setHover((h) => (h && "node" in h && h.node === n.id ? undefined : h))
              }
              onClick={(ev) => {
                ev.stopPropagation();
                if (pz.dragged() || edit) return;
                if (ev.metaKey || ev.ctrlKey) onSelect(n.id);
                else onOpen(n.id);
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
              {edit?.fresh.has(n.id) && <span className="cv-fresh" />}
            </button>
          );
        })}
        <div key={`${props.focus ?? "rest"}-over`} className={focus ? "sys-flinks" : undefined}>
          <MapEdges layer="over" {...edgeProps} />
          <EdgePills
            wires={wires}
            labels={props.labels}
            selectedEdge={selectedEdge}
            hotColor={hotColor}
            onHover={focus ? (edge) => setHover(edge ? { edge } : undefined) : undefined}
          />
        </div>
        {props.over}
      </div>
      {props.screen?.(toScreen, toWorld)}
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
