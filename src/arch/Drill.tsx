import { ReactFlowProvider, ViewportPortal } from "@xyflow/react";
import { useCallback, useMemo } from "react";
import type { ArchDto, GraphDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import type { Placed } from "../graph/layout";
import { Flow } from "../graph/SymbolFlow";
import type { Box } from "../graph/tree";
import { useGraph } from "../graph/useGraph";
import { useView } from "../graph/useView";
import { nextCallers, setAllExpanded, toggleAllCallers } from "../graph/view";
import type { CurrentReview } from "../review/current";
import type { CtxEntry } from "../ui/contextMenu";
import { DRAG_TYPE, droppedItem, startItemDrag } from "./ArchNode";
import { arrowHead, roundedPath } from "./paths";
import { moveSource } from "./scene";
import { scopedTree, scopeGraph } from "./scope";

interface Props {
  review: CurrentReview;
  arch: ArchDto;
  component: string;
  workspace: WorkspaceName;
  onStub(component: string): void;
  onStubOpen(component: string): void;
  stubMenu(component: string): CtxEntry[];
  onMove(path: string, symbol: string | null, to: string): void;
}

/** One component's symbols, as the symbol graph draws them, with the components they link
 * to as stubs. */
export function Drill(p: Props) {
  const { graph, error, loading } = useGraph(p.review);
  if (error) return <div className="placeholder error">{error}</div>;
  if (loading || !graph) return <div className="placeholder">Finding symbols…</div>;
  return <Scoped {...p} full={graph} />;
}

function Scoped({ full, ...p }: Props & { full: GraphDto }) {
  const { review, arch, component, workspace } = p;
  const scoped = useMemo(() => scopeGraph(full, arch, component), [full, arch, component]);
  const tree = useMemo(() => scopedTree(scoped), [scoped]);
  const [view, setView] = useView(
    `${workspace}\n${review.revset}\n${component}`,
    scoped.graph,
    tree,
  );
  const stubOf = useCallback(
    (box: Box) => (box.own.length === 1 ? scoped.stubs.get(box.own[0]) : undefined),
    [scoped],
  );
  const wrongTo = useMemo(
    () =>
      new Set(arch.edges.filter((e) => e.from === component && e.wrong !== null).map((e) => e.to)),
    [arch, component],
  );
  /** Move intents of this component's items: `path#symbol` or path, to where they belong. */
  const moves = useMemo(() => {
    const out = new Map<string, string>();
    for (const it of arch.intents) {
      if (it.kind !== "move" || it.state === "met" || moveSource(arch, it) !== component) continue;
      out.set(it.symbol ? `${it.path}#${it.symbol}` : it.path, it.to);
    }
    return out;
  }, [arch, component]);
  const statusOf = useMemo(() => new Map(arch.components.map((c) => [c.id, c.status])), [arch]);

  const decorate = useCallback(
    (box: Box) => {
      const stub = stubOf(box);
      if (stub !== undefined) {
        const tone = wrongTo.has(stub)
          ? "bad"
          : [...moves.values()].includes(stub)
            ? "move"
            : statusOf.get(stub) === "impacted"
              ? "imp"
              : "";
        return {
          extra: `astub ${tone}`,
          dropType: DRAG_TYPE,
          onDrop: (e: React.DragEvent) => {
            const item = droppedItem(e);
            if (item) p.onMove(item.path, item.symbol, stub);
          },
        };
      }
      if (box.kind !== "symbol" && box.kind !== "file") return {};
      const path = box.path as string;
      const symbol = box.kind === "symbol" ? box.label : null;
      const misplaced = moves.has(symbol ? `${path}#${symbol}` : path);
      return {
        extra: misplaced ? "amisplaced" : undefined,
        onDragStart: (e: React.DragEvent) => startItemDrag(e, { path, symbol }),
      };
    },
    [stubOf, wrongTo, moves, statusOf, p.onMove],
  );

  const overlay = (placed: Placed) => {
    let d = "";
    let arrows = "";
    for (const [item, to] of moves) {
      const [path, symbol] = item.split("#");
      const fromBox = [...tree.boxes.values()].find(
        (b) =>
          b.path === path && (symbol ? b.kind !== "file" && b.label === symbol : b.kind === "file"),
      );
      // A closed box around the item stands in for it.
      let shown = fromBox;
      while (shown && !placed.boxes.has(shown.id)) shown = tree.boxes.get(shown.parent ?? "");
      const stubId = [...scoped.stubs].find(([, c]) => c === to)?.[0];
      const a = shown && placed.boxes.get(shown.id)?.abs;
      const b = stubId !== undefined && placed.boxes.get(tree.boxOf[stubId])?.abs;
      if (!a || !b) continue;
      const right = b.x >= a.x + a.width;
      const pts = [
        { x: right ? a.x + a.width : a.x, y: a.y + 20 },
        { x: right ? b.x - 2 : b.x + b.width + 2, y: b.y + 20 },
      ];
      d += roundedPath(pts);
      arrows += arrowHead(pts[0], pts[1]);
    }
    return (
      <ViewportPortal>
        <svg className="aedges" aria-hidden="true">
          <g className="edge move">
            <path className="aline-path" d={d} />
            <path className="ahead" d={arrows} />
          </g>
        </svg>
      </ViewportPortal>
    );
  };

  const next = nextCallers(scoped.graph);
  const callersOn = scoped.graph.nodes.every(
    (n) => n.hop !== 0 || !next.has(n.id) || view.callersOpen.has(n.id),
  );
  const paneItems = (): CtxEntry[] => [
    { label: "Expand All", onSelect: () => setView(setAllExpanded(tree, view, true)) },
    { label: "Collapse All", onSelect: () => setView(setAllExpanded(tree, view, false)) },
    {
      label: "Callers",
      checked: next.size > 0 && callersOn,
      disabled: next.size === 0,
      onSelect: () => setView(toggleAllCallers(scoped.graph, tree, view, !callersOn)),
    },
  ];
  return (
    <ReactFlowProvider>
      <Flow
        review={review}
        graph={scoped.graph}
        tree={tree}
        view={view}
        setView={setView}
        workspace={workspace}
        paneItems={paneItems}
        decorate={decorate}
        overlay={overlay}
        onBox={(box) => {
          const stub = stubOf(box);
          if (stub !== undefined) p.onStub(stub);
          return stub !== undefined;
        }}
        onBoxOpen={(box) => {
          const stub = stubOf(box);
          if (stub !== undefined) p.onStubOpen(stub);
        }}
        menu={(box) => {
          const stub = stubOf(box);
          return stub === undefined ? undefined : p.stubMenu(stub);
        }}
      />
    </ReactFlowProvider>
  );
}
