import { type Node, ReactFlow, ReactFlowProvider, useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useEffect, useMemo, useState } from "react";
import type { GraphDto } from "../bindings";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { isMac } from "../workbench/keys";
import { useWorkspace, type WorkspaceName } from "../workspaces/context";
import { EdgesLayer } from "./EdgesLayer";
import { GraphBar, ZoomControls } from "./GraphBar";
import { layout, type Placed, type Scene } from "./layout";
import { showNode } from "./nodes";
import { route } from "./ports";
import { type BoxData, BoxNode, OpenBoxNode } from "./SymbolNode";
import { boxStats, jumpToBox } from "./stats";
import { buildTree, type Tree } from "./tree";
import { useGraph } from "./useGraph";
import { useView } from "./useView";
import {
  canExpand,
  links,
  nextCallers,
  presentBoxes,
  presentNodes,
  setAllExpanded,
  toggleAllCallers,
  toggleCallers,
  type View,
  visibleBoxes,
} from "./view";
import "./graph.css";

const nodeTypes = { box: BoxNode, open: OpenBoxNode };

function usePlaced(scene: Scene) {
  const [placed, setPlaced] = useState<{ scene: Scene; placed: Placed }>();
  useEffect(() => {
    let live = true;
    layout(scene).then((p) => live && setPlaced({ scene, placed: p }));
    return () => {
      live = false;
    };
  }, [scene]);
  return placed?.scene === scene ? placed.placed : undefined;
}

function useScene(graph: GraphDto, tree: Tree, view: View) {
  return useMemo(() => {
    const present = presentNodes(graph, view.callersOpen);
    const shown = presentBoxes(tree, present);
    const visible = visibleBoxes(tree, shown, view.expanded);
    const open = new Set(
      visible.filter((id) => view.expanded.has(id) && canExpand(tree, shown, id)),
    );
    const next = nextCallers(graph);
    const badges = new Set(
      visible.filter((id) => tree.boxes.get(id)?.own.some((n) => present[n] && next.has(n))),
    );
    const scene: Scene = {
      tree,
      visible,
      open,
      badges,
      routed: route(tree, links(graph, tree, present, view.expanded)),
    };
    return { scene, present, shown, next };
  }, [graph, tree, view]);
}

interface FlowProps {
  review: CurrentReview;
  graph: GraphDto;
  tree: Tree;
  view: View;
  setView(view: View): void;
  workspace: WorkspaceName;
}

function Flow({ review, graph, tree, view, setView, workspace }: FlowProps) {
  const { scene, present, shown, next } = useScene(graph, tree, view);
  const placed = usePlaced(scene);
  const flow = useReactFlow();
  const [hover, setHover] = useState<string>();
  const [selected, setSelected] = useState<string>();
  const nodes = useMemo(() => {
    if (!placed) return [];
    const stats = boxStats(review, graph, tree, present);
    const out: Node[] = [];
    for (const id of scene.visible) {
      const box = tree.boxes.get(id);
      const at = placed.boxes.get(id);
      if (!box || !at) continue;
      const s = stats.get(id);
      const own = box.own.filter((n) => present[n]);
      const ownNode = own.length > 0 ? graph.nodes[own[0]] : undefined;
      const callers = own.reduce((sum, n) => sum + (next.get(n)?.length ?? 0), 0);
      const data: BoxData = {
        box,
        expandable: canExpand(tree, shown, id),
        status: ownNode?.status,
        outside: s?.outside ?? false,
        count: s?.count ?? 0,
        done: s && s.all > 0 ? s.done / s.all : undefined,
        risk: s?.risk,
        callers,
        callersOpen: own.some((n) => view.callersOpen.has(n)),
        onCallers: () => {
          let v = view;
          for (const n of own) if (next.has(n)) v = toggleCallers(graph, tree, v, n);
          setView(v);
        },
        title: ownNode
          ? `${ownNode.path}:${ownNode.line}, ${ownNode.status}`
          : (box.path ?? box.label),
      };
      out.push({
        id,
        type: at.open ? "open" : "box",
        parentId: box.parent && placed.boxes.has(box.parent) ? box.parent : undefined,
        position: { x: at.at.x, y: at.at.y },
        selected: id === selected,
        style: { width: at.at.width, height: at.open ? at.at.height : undefined },
        data,
      });
    }
    return out;
  }, [review, graph, tree, view, setView, scene, placed, present, shown, next, selected]);
  // Fit when the boxes change, not on every progress update.
  useEffect(() => {
    if (placed) requestAnimationFrame(() => flow.fitView({ padding: 0.1 }));
  }, [placed, flow]);
  if (!placed) return <div className="placeholder">Laying out…</div>;
  return (
    <ReactFlow
      nodes={nodes}
      nodeTypes={nodeTypes}
      colorMode="dark"
      nodesDraggable={false}
      nodesConnectable={false}
      minZoom={0.1}
      onNodeClick={(e, n) => {
        const { box, expandable } = n.data as BoxData;
        setSelected(n.id);
        if (isMac ? e.metaKey : e.ctrlKey) return jumpToBox(review, graph, tree, box, workspace);
        if (expandable) {
          const expanded = new Set(view.expanded);
          if (!expanded.delete(box.id)) expanded.add(box.id);
          return setView({ ...view, expanded });
        }
        const own = box.own[0];
        if (own !== undefined) showNode(review, graph.nodes[own], workspace);
      }}
      onPaneClick={() => setSelected(undefined)}
      onNodeMouseEnter={(_, n) => setHover(n.id)}
      onNodeMouseLeave={() => setHover(undefined)}
    >
      <EdgesLayer routed={scene.routed} placed={placed} hover={hover} selected={selected} />
      <ZoomControls />
    </ReactFlow>
  );
}

/** The current review's changed symbols and how they use each other, nested in their
 * folders, files and enclosing symbols. Click a box to open it, a symbol to show it in the
 * review, and a symbol's callers badge to bring in the unchanged code that uses it. */
export function GraphPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const { graph, error, loading } = useGraph(review);
  if (!review) return <div className="placeholder">No review open</div>;
  if (error) return <div className="placeholder error">{error}</div>;
  if (loading || !graph) return <div className="placeholder">Finding symbols…</div>;
  if (!graph.nodes.some((n) => n.hop === 0)) {
    return <div className="placeholder">No changed symbols in {review.revset}</div>;
  }
  return <Loaded review={review} graph={graph} workspace={workspace} />;
}

function Loaded({
  review,
  graph,
  workspace,
}: {
  review: CurrentReview;
  graph: GraphDto;
  workspace: WorkspaceName;
}) {
  const tree = useMemo(() => buildTree(graph), [graph]);
  const [view, setView] = useView(`${workspace}\n${review.revset}`, graph, tree);
  const changed = graph.nodes.filter((n) => n.hop === 0).length;
  const firstHop = graph.nodes.filter((n) => n.hop === 1).length;
  const next = nextCallers(graph);
  const allOpen = graph.nodes.every(
    (n) => n.hop !== 0 || !next.has(n.id) || view.callersOpen.has(n.id),
  );
  return (
    <div className="graph-panel">
      <GraphBar
        changed={changed}
        callers={firstHop}
        callersOpen={allOpen}
        onCallers={(open) => setView(toggleAllCallers(graph, tree, view, open))}
        onExpandAll={() => setView(setAllExpanded(tree, view, true))}
        onCollapseAll={() => setView(setAllExpanded(tree, view, false))}
      />
      <div className="graph-canvas">
        <ReactFlowProvider>
          <Flow
            review={review}
            graph={graph}
            tree={tree}
            view={view}
            setView={setView}
            workspace={workspace}
          />
        </ReactFlowProvider>
      </div>
    </div>
  );
}
