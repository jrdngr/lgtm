import { message } from "@tauri-apps/plugin-dialog";
import { ReactFlowProvider } from "@xyflow/react";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import type { ArchDto } from "../bindings";
import { errorText } from "../ipc";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { useSendFromMenu } from "../review/useSendText";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { useWorkspace, type WorkspaceName } from "../workspaces/context";
import { ArchBar, type Crumb } from "./ArchBar";
import { ArchDetail } from "./ArchDetail";
import { ArchGraph } from "./ArchGraph";
import { componentView, containerView, droppedItem, type NodeView } from "./ArchNode";
import { addIntent, componentItems, edgeItems, edgeText, markWrong, removeIntent } from "./actions";
import { Drill } from "./Drill";
import { fixIntents, useArchRun } from "./runs";
import { buildScene, containerEdges, focusOf, type Mode, type SceneEdge } from "./scene";
import { type ArchSelection, type ArchView, setArchView, useArch } from "./state";
import "../graph/graph.css";
import "./arch.css";

type Level =
  | { kind: "containers" }
  | { kind: "components" }
  | { kind: "symbols"; component: string };

/** The review's architecture: containers › components › one component's symbols, with the
 * selection's detail on the right. Follows the current review and the commits picked in
 * History. */
export function ArchPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const { arch, error, view } = useArch(review);
  if (!review) return <div className="placeholder">No review open</div>;
  if (error) return <div className="placeholder error">{error}</div>;
  if (!arch) return <div className="placeholder">Reading the architecture…</div>;
  if (arch.components.length === 0) {
    return <div className="placeholder">No components in {review.revset}</div>;
  }
  return <Loaded key={review.id} review={review} arch={arch} view={view} workspace={workspace} />;
}

interface LoadedProps {
  review: CurrentReview;
  arch: ArchDto;
  view: ArchView;
  workspace: WorkspaceName;
}

function Loaded({ review, arch, view, workspace }: LoadedProps) {
  const id = review.id;
  const [picked, setLevel] = useState<Level>({ kind: "components" });
  // An edit or another range can take away the component drilled into.
  const level: Level =
    picked.kind === "symbols" && !arch.components.some((c) => c.id === picked.component)
      ? { kind: "components" }
      : picked;
  const [mode, setMode] = useState<Mode>("changed");
  const [center, setCenter] = useState<string[]>();
  const { sendText, picker } = useSendFromMenu(workspace);
  const fixing = useArchRun("intentFix", id)?.running ?? false;
  const scene = useMemo(() => buildScene(arch, mode), [arch, mode]);
  const sel = view.selection;
  const focus = useMemo(
    () => (level.kind === "components" ? focusOf(arch, scene.edges, sel) : undefined),
    [arch, scene, sel, level.kind],
  );

  /** Selections the tab makes, which shouldn't move the graph. */
  const own = useRef<ArchSelection | undefined>(undefined);
  const select = (selection: ArchSelection | undefined) => {
    own.current = selection;
    setArchView(id, { selection });
  };
  // A selection from the sidebar: show it in the components graph, in All if Changed hides it.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only a new selection moves the graph
  useEffect(() => {
    if (!sel || sel === own.current) return;
    const ids = [...(focusOf(arch, [], sel)?.nodes ?? [])];
    const drilled = level.kind === "symbols" && sel.kind === "component";
    if (!drilled) setLevel({ kind: "components" });
    if (ids.some((c) => !buildScene(arch, mode).components.some((x) => x.id === c))) {
      setMode("all");
    }
    setCenter(ids);
  }, [sel]);

  const send = (text: Promise<string>) =>
    text.then((t) => sendText(t)).catch((e) => message(errorText(e), { kind: "error" }));
  const edgeOf = (key: string): SceneEdge | undefined => scene.edges.find((e) => e.key === key);
  const sendSelection = () => {
    if (sel?.kind === "edge") {
      const e = arch.edges.find((x) => x.from === sel.from && x.to === sel.to);
      if (e) send(edgeText(id, arch, e));
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    const mod = e.metaKey || e.ctrlKey;
    if (!mod && e.key === "w" && sel?.kind === "edge") {
      const edge = arch.edges.find((x) => x.from === sel.from && x.to === sel.to);
      if (edge && edge.wrong === null) markWrong(id, edge.from, edge.to);
    } else if (mod && e.shiftKey && (e.key === ">" || e.key === ".")) sendSelection();
    else if (e.key === "Escape" && sel) select(undefined);
    else return;
    e.preventDefault();
  };

  const drop = (to: string) => (e: React.DragEvent) => {
    const item = droppedItem(e);
    const from = item && arch.components.find((c) => c.files.some((f) => f.path === item.path));
    if (item && from?.id !== to) {
      addIntent(id, { kind: "move", path: item.path, symbol: item.symbol, to });
    }
  };
  // biome-ignore lint/correctness/useExhaustiveDependencies: `drop` uses the review id and `arch`, which `scene` follows
  const comps = useMemo<NodeView[]>(
    () => scene.components.map((c) => ({ ...componentView(review, c), onDrop: drop(c.id) })),
    [scene, review],
  );
  const containers = useMemo(() => {
    const nodes = scene.groups.map((g) =>
      containerView(
        g.id,
        g.label,
        scene.components.filter((c) => g.children.includes(c.id)),
      ),
    );
    return { nodes, edges: containerEdges(arch, scene.edges) };
  }, [arch, scene]);

  const name = (c: string) => arch.components.find((x) => x.id === c)?.name ?? c;
  const crumbs: Crumb[] = (() => {
    const toComponents = () => setLevel({ kind: "components" });
    if (level.kind === "containers") return [{ label: "Containers" }];
    if (level.kind === "symbols") {
      const c = arch.components.find((x) => x.id === level.component);
      const box = arch.containers.find((x) => x.id === c?.container);
      return [
        { label: box?.name || "Components", onClick: toComponents },
        { label: name(level.component) },
      ];
    }
    return arch.containers.length > 1
      ? [
          { label: "Containers", onClick: () => setLevel({ kind: "containers" }) },
          { label: "Components" },
        ]
      : [];
  })();

  const componentMenu = (e: React.MouseEvent, c: string) => {
    const dto = arch.components.find((x) => x.id === c);
    if (!dto) return;
    select({ kind: "component", id: c });
    openMenu(e, componentItems(id, arch, dto));
  };
  const edgeMenu = (e: React.MouseEvent, key: string) => {
    const edge = edgeOf(key);
    if (edge?.intent !== undefined) {
      const intent = edge.intent;
      select({ kind: "intent", id: intent });
      return openMenu(e, [{ label: "Unmark", onSelect: () => removeIntent(id, intent) }]);
    }
    const dto = edge?.edge;
    if (!dto) return;
    select({ kind: "edge", from: dto.from, to: dto.to });
    openMenu(
      e,
      edgeItems(id, dto, () => send(edgeText(id, arch, dto))),
    );
  };
  const paneItems: CtxEntry[] = [];

  const graph =
    level.kind === "symbols" ? (
      <Drill
        review={review}
        arch={arch}
        component={level.component}
        workspace={workspace}
        onStub={(c) => select({ kind: "component", id: c })}
        onStubOpen={(c) => setLevel({ kind: "symbols", component: c })}
        stubMenu={(c) => {
          const dto = arch.components.find((x) => x.id === c);
          return dto ? componentItems(id, arch, dto) : [];
        }}
        onMove={(path, symbol, to) => addIntent(id, { kind: "move", path, symbol, to })}
      />
    ) : level.kind === "containers" ? (
      <ReactFlowProvider>
        <ArchGraph
          nodes={containers.nodes}
          groups={[]}
          edges={containers.edges}
          boundaries={[]}
          onNode={() => {}}
          onNodeOpen={(g) => {
            setLevel({ kind: "components" });
            setCenter(scene.groups.find((x) => x.id === g)?.children);
          }}
          onNodeMenu={() => {}}
          onEdge={() => {}}
          onEdgeMenu={() => {}}
          onPane={() => {}}
          paneItems={paneItems}
        />
      </ReactFlowProvider>
    ) : (
      <ReactFlowProvider>
        <ArchGraph
          nodes={comps}
          groups={scene.groups}
          edges={scene.edges}
          boundaries={scene.boundaries}
          focus={focus}
          selected={sel?.kind === "component" ? sel.id : undefined}
          center={center}
          onNode={(c) => select({ kind: "component", id: c })}
          onNodeOpen={(c) => setLevel({ kind: "symbols", component: c })}
          onNodeMenu={componentMenu}
          onEdge={(key) => {
            const e = edgeOf(key);
            if (e?.intent !== undefined) select({ kind: "intent", id: e.intent });
            else if (e) select({ kind: "edge", from: e.from, to: e.to });
          }}
          onEdgeMenu={edgeMenu}
          onPane={() => select(undefined)}
          paneItems={paneItems}
        />
      </ReactFlowProvider>
    );

  return (
    <div className="graph-panel arch-panel" tabIndex={-1} onKeyDown={onKey}>
      <ArchBar
        crumbs={crumbs}
        mode={level.kind === "symbols" ? undefined : mode}
        onMode={setMode}
        range={view.range}
        onClearRange={() => setArchView(id, { range: undefined })}
        counts={arch.counts}
        transitive={view.transitive}
        onTransitive={(transitive) => setArchView(id, { transitive })}
      />
      <div className="awork">
        <div className="graph-canvas acanvas">{graph}</div>
        {sel && (
          <ArchDetail
            review={review}
            arch={arch}
            selection={sel}
            onSelect={select}
            onClose={() => select(undefined)}
            send={send}
            markWrong={(e) => markWrong(id, e.from, e.to)}
            unmark={(i) => removeIntent(id, i)}
            fix={(ids) => fixIntents(workspace, id, review.revset, ids)}
            fixing={fixing}
          />
        )}
      </div>
      {picker}
    </div>
  );
}
