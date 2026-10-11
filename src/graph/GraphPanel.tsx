import { Background, type Node, ReactFlow, ReactFlowProvider, useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useEffect, useMemo, useState } from "react";
import type { GraphDto } from "../bindings";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";
import { EdgesLayer } from "./EdgesLayer";
import { layout, type Placed } from "./layout";
import { nodeDone, nodeRisk, showNode } from "./nodes";
import { FileNode, type SymbolData, SymbolNode } from "./SymbolNode";
import { useGraph } from "./useGraph";
import "./graph.css";

const nodeTypes = { symbol: SymbolNode, file: FileNode };

function usePlaced(graph: GraphDto | undefined) {
  const [placed, setPlaced] = useState<{ graph: GraphDto; placed: Placed }>();
  useEffect(() => {
    if (!graph) return;
    let live = true;
    layout(graph).then((p) => live && setPlaced({ graph, placed: p }));
    return () => {
      live = false;
    };
  }, [graph]);
  return placed?.graph === graph ? placed?.placed : undefined;
}

function toNodes(review: CurrentReview, graph: GraphDto, placed: Placed) {
  const nodes: Node[] = [];
  for (const [path, box] of placed.groups) {
    const outside = graph.nodes.every((n) => n.path !== path || n.file === null);
    nodes.push({
      id: `file:${path}`,
      type: "file",
      position: { x: box.x, y: box.y },
      style: { width: box.width, height: box.height },
      data: { path, outside },
      selectable: false,
    });
  }
  for (const n of graph.nodes) {
    const box = placed.nodes.get(n.id);
    if (!box) continue;
    const data: SymbolData = { node: n, done: nodeDone(review, n), risk: nodeRisk(review, n) };
    nodes.push({
      id: String(n.id),
      type: "symbol",
      parentId: `file:${n.path}`,
      position: { x: box.x, y: box.y },
      style: { width: box.width },
      data,
    });
  }
  return nodes;
}

function Flow({ review, graph }: { review: CurrentReview; graph: GraphDto }) {
  const placed = usePlaced(graph);
  const flow = useReactFlow();
  const workspace = useWorkspace();
  const [focus, setFocus] = useState<number>();
  const nodes = useMemo(
    () => (placed ? toNodes(review, graph, placed) : []),
    [review, graph, placed],
  );
  // Fit once per layout, not on every progress update.
  useEffect(() => {
    if (placed) requestAnimationFrame(() => flow.fitView({ padding: 0.1 }));
  }, [placed, flow]);
  if (!placed) return <div className="placeholder">Laying out…</div>;
  const symbol = (n: Node) => (n.type === "symbol" ? (n.data as SymbolData).node : undefined);
  return (
    <ReactFlow
      nodes={nodes}
      nodeTypes={nodeTypes}
      colorMode="dark"
      nodesDraggable={false}
      nodesConnectable={false}
      minZoom={0.1}
      onNodeClick={(_, n) => {
        const s = symbol(n);
        if (s) showNode(review, s, workspace);
      }}
      onNodeMouseEnter={(_, n) => setFocus(symbol(n)?.id)}
      onNodeMouseLeave={() => setFocus(undefined)}
    >
      <EdgesLayer graph={graph} placed={placed} focus={focus} />
      <Background gap={24} />
    </ReactFlow>
  );
}

/** Past this many callers they start hidden, since they crowd out the changed symbols. */
const MAX_SHOWN_CALLERS = 60;

/** `graph` without its callers, keeping node ids as indexes. */
function withoutCallers(graph: GraphDto): GraphDto {
  const keep = graph.nodes.filter((n) => n.status !== "caller");
  const ids = new Set(keep.map((n) => n.id));
  return { nodes: keep, edges: graph.edges.filter((e) => ids.has(e.from) && ids.has(e.to)) };
}

/** The current review's changed symbols, how they use each other, and their callers. Click
 * a symbol to show it in the review. */
export function GraphPanel() {
  const review = useCurrentReview(useWorkspace());
  const { graph: full, error, loading } = useGraph(review);
  const callers = full?.nodes.filter((n) => n.status === "caller").length ?? 0;
  const [showCallers, setShowCallers] = useState<boolean>();
  const shown = showCallers ?? callers <= MAX_SHOWN_CALLERS;
  const graph = useMemo(() => full && (shown ? full : withoutCallers(full)), [full, shown]);
  if (!review) return <div className="placeholder">No review open</div>;
  if (error) return <div className="placeholder error">{error}</div>;
  if (loading || !graph) return <div className="placeholder">Finding symbols…</div>;
  if (graph.nodes.length === 0) {
    return <div className="placeholder">No changed symbols in {review.revset}</div>;
  }
  return (
    <div className="graph-panel">
      <div className="graph-bar">
        <span>{graph.nodes.length - (shown ? callers : 0)} changed symbols</span>
        <label>
          <input
            type="checkbox"
            checked={shown}
            disabled={callers === 0}
            onChange={(e) => setShowCallers(e.target.checked)}
          />
          {callers} unchanged callers
        </label>
      </div>
      <div className="graph-canvas">
        <ReactFlowProvider>
          <Flow review={review} graph={graph} />
        </ReactFlowProvider>
      </div>
    </div>
  );
}
