import { type Node, ReactFlow, useReactFlow } from "@xyflow/react";
import { type MouseEvent, useEffect, useMemo, useRef, useState } from "react";
import { ZoomControls } from "../graph/ZoomControls";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { ArchEdges } from "./ArchEdges";
import { ArchNode, type NodeView } from "./ArchNode";
import { type LayoutInput, layoutArch, type Placed } from "./layout";
import { edgeAt } from "./paths";
import type { Focus, SceneEdge, SceneGroup } from "./scene";

const nodeTypes = { arch: ArchNode };

/** The last finished layout, kept while the next one runs so the canvas stays up. */
function usePlaced(input: LayoutInput) {
  const [placed, setPlaced] = useState<{ input: LayoutInput; placed: Placed }>();
  useEffect(() => {
    let live = true;
    layoutArch(input).then((p) => live && setPlaced({ input, placed: p }));
    return () => {
      live = false;
    };
  }, [input]);
  return placed;
}

export interface GraphProps {
  nodes: NodeView[];
  groups: SceneGroup[];
  edges: SceneEdge[];
  boundaries: { name: string; components: string[] }[];
  focus?: Focus;
  /** The selected box, outlined. */
  selected?: string;
  /** Boxes to bring into view when they change. */
  center?: string[];
  onNode(id: string): void;
  onNodeOpen(id: string): void;
  onNodeMenu(e: MouseEvent, id: string): void;
  onEdge(key: string): void;
  onEdgeMenu(e: MouseEvent, key: string): void;
  onPane(): void;
  /** The background menu's items after Fit to Window. */
  paneItems: CtxEntry[];
}

/** Boxes laid out by ELK, in their groups, with the edges drawn under them. Edges are found
 * by distance from the pointer, since they share paths. */
export function ArchGraph(p: GraphProps) {
  // By ids, so new review progress in the boxes doesn't lay out and fit again.
  const ids = p.nodes.map((n) => n.id).join("\n");
  const input = useMemo<LayoutInput>(
    () => ({ groups: p.groups, nodes: ids ? ids.split("\n") : [], edges: p.edges }),
    [p.groups, ids, p.edges],
  );
  const placed = usePlaced(input)?.placed;
  const flow = useReactFlow();
  const wrap = useRef<HTMLDivElement>(null);
  const nodes = useMemo<Node[]>(() => {
    if (!placed) return [];
    return p.nodes.flatMap((n) => {
      const at = placed.nodes.get(n.id);
      if (!at) return [];
      const f = p.focus;
      const lit = f?.nodes.has(n.id) && (f.primary === undefined || f.primary === n.id);
      const focus = f && (lit ? "sel" : f.nodes.has(n.id) ? undefined : "dimn");
      return [
        {
          id: n.id,
          type: "arch",
          position: { x: at.x, y: at.y },
          selected: n.id === p.selected,
          data: { ...n, focus },
        },
      ];
    });
  }, [placed, p.nodes, p.focus, p.selected]);

  useEffect(() => {
    if (placed) requestAnimationFrame(() => flow.fitView({ padding: 0.1, maxZoom: 1 }));
  }, [placed, flow]);
  const center = p.center?.join("\n");
  useEffect(() => {
    if (!placed || !center) return;
    const ids = center.split("\n").filter((id) => placed.nodes.has(id));
    if (ids.length === 0) return;
    requestAnimationFrame(() =>
      flow.fitView({ nodes: ids.map((id) => ({ id })), padding: 0.4, maxZoom: 1, duration: 200 }),
    );
  }, [placed, center, flow]);

  if (!placed) return <div className="placeholder">Laying out…</div>;

  const edgeUnder = (e: MouseEvent) => {
    const at = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    return edgeAt(p.edges, placed.routes, at, 6 / flow.getZoom());
  };
  return (
    <div className="arch-flow" ref={wrap}>
      <ReactFlow
        nodes={nodes}
        nodeTypes={nodeTypes}
        colorMode="dark"
        nodesDraggable={false}
        nodesConnectable={false}
        zoomOnDoubleClick={false}
        minZoom={0.1}
        onNodeClick={(_, n) => p.onNode(n.id)}
        onNodeDoubleClick={(_, n) => p.onNodeOpen(n.id)}
        onNodeContextMenu={(e, n) => p.onNodeMenu(e, n.id)}
        onPaneClick={(e) => {
          const key = edgeUnder(e);
          if (key) p.onEdge(key);
          else p.onPane();
        }}
        onPaneContextMenu={(e) => {
          const key = edgeUnder(e as MouseEvent);
          if (key) return p.onEdgeMenu(e as MouseEvent, key);
          openMenu(e, [
            { label: "Fit to Window", onSelect: () => flow.fitView({ padding: 0.1 }) },
            ...p.paneItems,
          ]);
        }}
        onPaneMouseMove={(e) => {
          if (wrap.current) wrap.current.style.cursor = edgeUnder(e) ? "pointer" : "";
        }}
      >
        <ArchEdges
          placed={placed}
          groups={p.groups}
          edges={p.edges}
          boundaries={p.boundaries}
          focus={p.focus}
        />
        <ZoomControls />
      </ReactFlow>
    </div>
  );
}
