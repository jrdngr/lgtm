import { type KeyboardEvent, useMemo, useState } from "react";
import { showInTab } from "../arch/sidebar/parts";
import { useArch } from "../arch/state";
import type { SystemDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useSendText } from "../review/useSendText";
import { PipeButton } from "../terminal/PipeButton";
import { IconButton } from "../ui/icons";
import { layout } from "./layout";
import { SystemCanvas } from "./SystemCanvas";
import { SystemDetail } from "./SystemDetail";
import {
  containerId,
  type Level,
  largestContainer,
  type Overlay,
  overlayOf,
  type SEdge,
  type SNode,
  SYSTEM,
  sceneOf,
} from "./scene";
import { useSystem } from "./useSystem";
import "../arch/arch.css";
import "./system.css";

type EdgeMode = "selected" | "changed" | "all";

const LEVELS = [
  ["context", "Context"],
  ["containers", "Containers"],
  ["components", "Components"],
] as const;

/** The whole system as C4 context, containers and components, from the workspace's working
 * copy and the description a terminal agent wrote. */
export function SystemPanel() {
  const workspace = useWorkspace();
  const { map, error, reload } = useSystem(workspace);
  if (error && !map) return <div className="placeholder error">{error}</div>;
  if (!map) return <div className="placeholder">Reading the system…</div>;
  if (!map.components.length) return <div className="placeholder">No source files</div>;
  return <Loaded map={map} error={error} reload={reload} />;
}

/** The node that stands for `level` when nothing is selected. */
function subject(map: SystemDto, level: Level): SNode | undefined {
  const holder =
    level.kind === "components"
      ? sceneOf(map, { kind: "containers" }).inside.flatMap((r) => r.nodes)
      : sceneOf(map, { kind: "context" }).inside.flatMap((r) => r.nodes);
  const id = level.kind === "components" ? containerId(level.container) : SYSTEM;
  return holder.find((n) => n.id === id);
}

/** With nothing selected, Selected shows described relations and this many of the heaviest
 * imports. */
const HEAVIEST = 12;

function visible(edges: SEdge[], mode: EdgeMode, selected?: string) {
  if (mode === "changed") return edges.filter((e) => e.status);
  if (mode === "all") return edges;
  if (selected) return edges.filter((e) => e.from === selected || e.to === selected);
  const heavy = edges.filter((e) => !e.label).sort((a, b) => b.imports - a.imports);
  const keep = new Set(heavy.slice(0, HEAVIEST));
  return edges.filter((e) => e.label || keep.has(e));
}

function Loaded({ map, error, reload }: { map: SystemDto; error?: string; reload(): void }) {
  const workspace = useWorkspace();
  const [level, setLevel] = useState<Level>({ kind: "context" });
  const [selected, setSelected] = useState<string>();
  const [mode, setMode] = useState<EdgeMode>("selected");
  const [change, setChange] = useState(false);
  const [sendError, setSendError] = useState<string>();
  const { sendText, picker } = useSendText(workspace, setSendError, () => {});
  const review = useCurrentReview(workspace);
  const { arch } = useArch(change ? review : undefined);
  const overlay: Overlay | undefined = useMemo(
    () => (change && arch ? overlayOf(arch) : undefined),
    [change, arch],
  );
  // The container drilled into can go when the map reloads.
  const shown: Level =
    level.kind === "components" && !map.containers.some((c) => c.id === level.container)
      ? { kind: "containers" }
      : level;
  const scene = useMemo(() => sceneOf(map, shown, overlay), [map, shown, overlay]);
  const placed = useMemo(() => layout(scene), [scene]);
  const nodes = placed.nodes;
  const sel = nodes.find((n) => n.id === selected);
  const detail = sel ?? subject(map, shown);
  const edgeMode: EdgeMode = mode === "changed" && !overlay ? "selected" : mode;
  const edges = shown.kind === "context" ? scene.edges : visible(scene.edges, edgeMode, sel?.id);

  const go = (next: Level, node?: string) => {
    setLevel(next);
    setSelected(node || undefined);
  };
  const container = level.kind === "components" ? level.container : largestContainer(map);
  const pickLevel = (kind: Level["kind"]) => {
    if (kind !== "components") return go({ kind });
    if (container !== undefined) go({ kind, container });
  };
  const open = (id: string) => {
    if (id === SYSTEM) return go({ kind: "containers" });
    if (id.startsWith("c:") && map.containers.some((c) => containerId(c.id) === id)) {
      go({ kind: "components", container: id.slice(2) });
    }
  };
  const up = () => {
    if (shown.kind === "components") go({ kind: "containers" }, containerId(shown.container));
    else if (shown.kind === "containers") go({ kind: "context" }, SYSTEM);
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && selected) setSelected(undefined);
    else if (e.key === "Escape" || e.key === "Backspace") up();
    else return;
    e.preventDefault();
  };
  async function describe() {
    const r = await commands.describePrompt(workspace);
    if (r.status === "error") return setSendError(r.error);
    setSendError(undefined);
    sendText(r.data);
  }
  const containerName =
    shown.kind === "components" ? (subject(map, shown)?.name ?? shown.container) : undefined;

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the tab takes Escape and Backspace
    <div className="arch-panel sys-panel" tabIndex={0} onKeyDown={onKey}>
      <div className="abar">
        <span className="acrumbs">
          <span className="acrumb">
            {shown.kind === "components" ? (
              <button type="button" className="alink" onClick={() => go({ kind: "containers" })}>
                {map.repo}
              </button>
            ) : (
              map.repo
            )}
          </span>
          {containerName && (
            <span className="acrumb">
              <i>›</i>
              {containerName}
            </span>
          )}
        </span>
        <span className="aseg">
          {LEVELS.map(([kind, label]) => (
            <button
              key={kind}
              type="button"
              className={shown.kind === kind ? "on" : undefined}
              aria-pressed={shown.kind === kind}
              onClick={() => pickLevel(kind)}
            >
              {label}
            </button>
          ))}
        </span>
        {shown.kind !== "context" && (
          <span className="aseg">
            {(overlay
              ? (["selected", "changed", "all"] as const)
              : (["selected", "all"] as const)
            ).map((m) => (
              <button
                key={m}
                type="button"
                className={edgeMode === m ? "on" : undefined}
                aria-pressed={edgeMode === m}
                onClick={() => setMode(m)}
              >
                {m === "selected" ? "Selected" : m === "changed" ? "Changed" : "All"}
              </button>
            ))}
          </span>
        )}
        <span className="sys-grow" />
        {map.description && map.stale.length > 0 && (
          <span className="apill amber">{map.stale.length} stale</span>
        )}
        {review && (
          <button
            type="button"
            className={`atog${change ? " on" : ""}`}
            aria-pressed={change}
            onClick={() => setChange(!change)}
          >
            <span className="atrack" />
            Change
          </button>
        )}
        <IconButton icon="sync" label="Reload" onClick={reload} />
        <PipeButton onClick={describe}>Describe</PipeButton>
      </div>
      {(error || sendError) && <div className="sys-error error">{sendError ?? error}</div>}
      <div className="sys-work">
        <SystemCanvas
          layout={placed}
          edges={edges}
          quiet={edgeMode === "all" && shown.kind !== "context"}
          selected={sel?.id}
          onSelect={setSelected}
          onOpen={open}
        />
        {detail && (
          <SystemDetail
            map={map}
            node={detail}
            nodes={nodes}
            edges={scene.edges}
            onPick={go}
            onArchitecture={
              overlay && review
                ? (id) => showInTab(workspace, review.id, { kind: "component", id })
                : undefined
            }
          />
        )}
      </div>
      {picker}
    </div>
  );
}
