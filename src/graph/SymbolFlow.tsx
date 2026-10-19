import { type Node, ReactFlow, useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { GraphDto } from "../bindings";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import type { CurrentReview } from "../review/current";
import { absolutePath } from "../services/pathMenu";
import { type CtxEntry, copy, openMenu } from "../ui/contextMenu";
import { EdgesLayer } from "./EdgesLayer";
import { layout, type Placed, type Scene } from "./layout";
import { showNode } from "./nodes";
import { route } from "./ports";
import { type BoxData, BoxNode, OpenBoxNode } from "./SymbolNode";
import { boxStats, jumpToBox } from "./stats";
import type { Box, Tree } from "./tree";
import {
  canExpand,
  links,
  nextCallers,
  presentBoxes,
  presentNodes,
  toggleCallers,
  type View,
  visibleBoxes,
} from "./view";
import { ZoomControls } from "./ZoomControls";
import "./graph.css";

const nodeTypes = { box: BoxNode, open: OpenBoxNode };

/** A box's file, or a folder's own path from its `d:` id. */
const boxPath = (box: Box) => box.path ?? box.id.slice(2);

/** The last finished layout, kept while the next one runs so the canvas stays up. */
function usePlaced(scene: Scene) {
  const [placed, setPlaced] = useState<{ scene: Scene; placed: Placed }>();
  useEffect(() => {
    let live = true;
    layout(scene).then((p) => live && setPlaced({ scene, placed: p }));
    return () => {
      live = false;
    };
  }, [scene]);
  return placed;
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

export interface FlowProps {
  review: CurrentReview;
  graph: GraphDto;
  tree: Tree;
  view: View;
  setView(view: View): void;
  workspace: WorkspaceName;
  /** The background menu's items after Fit to Window. */
  paneItems(): CtxEntry[];
  /** Handles a click on a box in place of showing it. Returns whether it did. */
  onBox?(box: Box): boolean;
  /** Double-click on a box. */
  onBoxOpen?(box: Box): void;
  /** A box's menu in place of its own. */
  menu?(box: Box): CtxEntry[] | undefined;
  /** More of a box's look and behavior, like classes and drag and drop. */
  decorate?(box: Box): Partial<BoxData>;
  /** Drawn over the graph once it's laid out. */
  overlay?(placed: Placed): ReactNode;
}

/** The symbol graph: boxes nested in their folders, files and enclosing symbols. Click a box
 * to open it, a symbol to show it in the review, and a symbol's callers badge to bring in the
 * unchanged code that uses it. */
export function Flow(p: FlowProps) {
  const { review, graph, tree, view, setView, workspace, paneItems, decorate } = p;
  const { scene: wanted, present, shown, next } = useScene(graph, tree, view);
  const { scene, placed } = usePlaced(wanted) ?? {};
  const flow = useReactFlow();
  const [hover, setHover] = useState<string>();
  const [selected, setSelected] = useState<string>();
  const nodes = useMemo(() => {
    if (!scene || !placed) return [];
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
        ...decorate?.(box),
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
  }, [review, graph, tree, view, setView, scene, placed, present, shown, next, selected, decorate]);
  // Fit when the boxes change, not on every progress update. A box can unmount under the
  // pointer without a mouse leave, so hover starts over.
  useEffect(() => {
    if (!placed) return;
    setHover(undefined);
    requestAnimationFrame(() => flow.fitView({ padding: 0.1 }));
  }, [placed, flow]);
  if (!scene || !placed) return <div className="placeholder">Laying out…</div>;

  const toggleOpen = (box: Box) => {
    const expanded = new Set(view.expanded);
    if (!expanded.delete(box.id)) expanded.add(box.id);
    setView({ ...view, expanded });
  };

  function boxItems(data: BoxData): CtxEntry[] {
    const { box } = data;
    return [
      {
        label: "Jump to Code",
        disabled: box.kind === "folder",
        onSelect: () => jumpToBox(review, graph, tree, box, present, workspace),
      },
      data.expandable && {
        label: view.expanded.has(box.id) ? "Close" : "Open",
        onSelect: () => toggleOpen(box),
      },
      data.callers > 0 && {
        label: data.callersOpen ? "Hide Callers" : "Show Callers",
        onSelect: data.onCallers,
      },
      "-",
      { label: "Copy Name", onSelect: () => copy(box.label) },
      { label: "Copy Path", onSelect: () => copy(absolutePath(workspace, boxPath(box))) },
    ];
  }

  return (
    <ReactFlow
      nodes={nodes}
      nodeTypes={nodeTypes}
      colorMode="dark"
      nodesDraggable={false}
      nodesConnectable={false}
      zoomOnDoubleClick={!p.onBoxOpen}
      minZoom={0.1}
      onNodeClick={(e, n) => {
        const { box, expandable } = n.data as BoxData;
        setSelected(n.id);
        if (p.onBox?.(box)) return;
        if (isMac ? e.metaKey : e.ctrlKey) {
          return jumpToBox(review, graph, tree, box, present, workspace);
        }
        if (expandable) return toggleOpen(box);
        const own = box.own[0];
        if (own !== undefined) showNode(review, graph.nodes[own], workspace);
      }}
      onNodeDoubleClick={(_, n) => p.onBoxOpen?.((n.data as BoxData).box)}
      onPaneClick={() => setSelected(undefined)}
      onNodeContextMenu={(e, n) => {
        setSelected(n.id);
        const data = n.data as BoxData;
        openMenu(e, p.menu?.(data.box) ?? boxItems(data));
      }}
      onPaneContextMenu={(e) =>
        openMenu(e, [
          { label: "Fit to Window", onSelect: () => flow.fitView({ padding: 0.1 }) },
          ...paneItems(),
        ])
      }
      onNodeMouseEnter={(_, n) => setHover(n.id)}
      onNodeMouseLeave={() => setHover(undefined)}
    >
      <EdgesLayer routed={scene.routed} placed={placed} hover={hover} selected={selected} />
      {p.overlay?.(placed)}
      <ZoomControls />
    </ReactFlow>
  );
}
