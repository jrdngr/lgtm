import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { useArch } from "../arch/state";
import type { SystemDto } from "../bindings";
import { revealInFiles } from "../core/reveal";
import { useWorkspace } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useSendText } from "../review/useSendText";
import { NEUTRAL } from "./colors";
import { FindPalette } from "./FindPalette";
import { layout } from "./layout";
import { Minimap } from "./Minimap";
import { nodeInfo } from "./names";
import { type Crumb, type EdgeMode, SystemBar } from "./SystemBar";
import { edgeKey, SystemCanvas } from "./SystemCanvas";
import { SystemDetail } from "./SystemDetail";
import {
  containerColors,
  containerId,
  type Level,
  largestContainer,
  levelOf,
  type Overlay,
  overlayOf,
  type SNode,
  SYSTEM,
  sceneOf,
} from "./scene";
import { onShowInSystem } from "./showInSystem";
import { TourCard, TourSteps, tourSteps } from "./Tour";
import { TracePanel } from "./TracePanel";
import { drawn, tracePath } from "./trace";
import { useSystem } from "./useSystem";
import { askPrompt, neighborhood, visibleEdges } from "./view";
import "../arch/arch.css";
import "./system.css";

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
  if (level.kind === "components") {
    const id = containerId(level.container);
    return sceneOf(map, { kind: "containers" })
      .inside.flatMap((r) => r.nodes)
      .find((n) => n.id === id);
  }
  return sceneOf(map, { kind: "context" }).inside[0]?.nodes[0];
}

interface Trace {
  from?: string;
  to?: string;
}

function Loaded({ map, error, reload }: { map: SystemDto; error?: string; reload(): void }) {
  const workspace = useWorkspace();
  const root = useRef<HTMLDivElement>(null);
  const [level, setLevel] = useState<Level>({ kind: "context" });
  const [selected, setSelected] = useState<string>();
  const [mode, setMode] = useState<EdgeMode>("focus");
  const [change, setChange] = useState(false);
  const [finding, setFinding] = useState(false);
  const [trace, setTrace] = useState<Trace>();
  const [tourAt, setTourAt] = useState<number>();
  const [sendError, setSendError] = useState<string>();
  const { sendText, picker } = useSendText(workspace, setSendError, () => root.current?.focus());
  const review = useCurrentReview(workspace);
  const { arch } = useArch(change ? review : undefined);
  const overlay: Overlay | undefined = useMemo(
    () => (change && arch ? overlayOf(arch) : undefined),
    [change, arch],
  );
  const colors = useMemo(() => containerColors(map), [map]);
  const steps = useMemo(() => tourSteps(map, colors), [map, colors]);

  // The container drilled into can go when the map reloads.
  const shown: Level =
    level.kind === "components" && !map.containers.some((c) => c.id === level.container)
      ? { kind: "containers" }
      : level;
  const scene = useMemo(() => sceneOf(map, shown, overlay), [map, shown, overlay]);
  const placed = useMemo(() => layout(scene), [scene]);
  const sel = placed.nodes.find((n) => n.id === selected);
  const detail = sel ?? subject(map, shown);
  const edgeMode: EdgeMode = mode === "changed" && !overlay ? "focus" : mode;
  const edges =
    shown.kind === "context" ? scene.edges : visibleEdges(scene.edges, edgeMode, sel?.id);

  const go = (next: Level, node?: string) => {
    setLevel(next);
    setSelected(node);
  };
  /** Shows `id` at the level that holds it, staying put when this level shows it. */
  const show = (id: string) => {
    if (placed.nodes.some((n) => n.id === id && !n.door)) return setSelected(id);
    const at = levelOf(map, id);
    if (at) go(at, id);
  };
  const open = (id: string) => {
    if (id === SYSTEM) return go({ kind: "containers" });
    const c = map.containers.find((c) => containerId(c.id) === id);
    if (c && map.components.some((k) => k.container === c.id)) {
      go({ kind: "components", container: c.id });
    }
  };
  const up =
    shown.kind === "components"
      ? () => go({ kind: "containers" }, containerId(shown.container))
      : shown.kind === "containers"
        ? () => go({ kind: "context" }, SYSTEM)
        : undefined;
  const container = shown.kind === "components" ? shown.container : largestContainer(map);
  const pickLevel = (kind: Level["kind"]) => {
    if (kind !== "components") return go({ kind });
    if (container !== undefined) go({ kind, container });
  };

  useEffect(
    () =>
      onShowInSystem(workspace, (id) => {
        const at = levelOf(map, id);
        if (at) {
          setLevel(at);
          setSelected(id);
        }
      }),
    [workspace, map],
  );

  // Each tour step shows its node.
  const step = tourAt !== undefined ? steps[tourAt] : undefined;
  // biome-ignore lint/correctness/useExhaustiveDependencies: moves only when the step does
  useEffect(() => {
    if (!step) return;
    const at = levelOf(map, step.id);
    if (at) go(at, step.id);
  }, [step?.id, tourAt]);

  const hops = trace?.from && trace.to ? (tracePath(map, trace.from, trace.to) ?? []) : undefined;
  const path = hops && drawn(map, shown, hops);
  const focus = step?.id ?? sel?.id;
  const lit = path
    ? path.nodes
    : step
      ? new Set([step.id])
      : edgeMode === "focus" && focus && shown.kind !== "context"
        ? neighborhood(scene.edges, focus)
        : undefined;
  const hot = new Map<string, number | undefined>();
  if (path) for (const [k, n] of path.edges) hot.set(k, n);
  else if (focus) {
    for (const e of scene.edges)
      if (e.from === focus || e.to === focus) hot.set(edgeKey(e), undefined);
  }
  const hotColor = path
    ? "#e6e6e6"
    : focus
      ? (nodeInfo(map, colors, focus)?.color ?? NEUTRAL)
      : NEUTRAL;
  const drawnEdges = path || step ? scene.edges.filter((e) => hot.has(edgeKey(e))) : edges;
  const stale = useMemo(() => {
    if (!map.description) return new Set<string>();
    const ids = new Set(map.stale.map((id) => `k:${id}`));
    for (const k of map.components) if (map.stale.includes(k.id)) ids.add(containerId(k.container));
    return ids;
  }, [map]);

  const pickNode = (id: string | undefined) => {
    const door = placed.nodes.find((n) => n.id === id)?.door;
    if (id && door && !trace) {
      return map.components.some((k) => containerId(k.container) === id) ? open(id) : show(id);
    }
    if (!trace || !id) return setSelected(id);
    if (!trace.from) setTrace({ ...trace, from: id });
    else setTrace({ ...trace, to: id });
  };
  const crumb = (id: string, onClick?: () => void): Crumb | undefined => {
    const info = nodeInfo(map, colors, id);
    return (
      info && {
        id,
        name: info.name,
        color: info.kind === "container" ? info.color : undefined,
        onClick,
      }
    );
  };
  const crumbs: Crumb[] = [
    { id: SYSTEM, name: map.repo, onClick: () => go({ kind: "context" }, SYSTEM) },
    ...(shown.kind === "components"
      ? [
          crumb(containerId(shown.container), () =>
            go({ kind: "components", container: shown.container }),
          ),
        ]
      : []),
    ...(shown.kind === "components" && sel ? [crumb(sel.id)] : []),
  ].filter((c): c is Crumb => !!c);

  const onFile = (p: string, line?: number) => openFile(workspace, p, line);
  async function describe() {
    const r = await commands.describePrompt(workspace);
    if (r.status === "error") return setSendError(r.error);
    setSendError(undefined);
    sendText(r.data);
  }
  const startTour = steps.length
    ? () => {
        setTrace(undefined);
        setTourAt(0);
      }
    : undefined;

  const onKey = (e: KeyboardEvent) => {
    if (finding) return;
    if (e.key === "/") setFinding(true);
    else if (step && e.key === "ArrowRight")
      setTourAt(Math.min(steps.length - 1, (tourAt ?? 0) + 1));
    else if (step && e.key === "ArrowLeft") setTourAt(Math.max(0, (tourAt ?? 0) - 1));
    else if (e.key === "Escape" && trace) setTrace(undefined);
    else if (e.key === "Escape" && step) setTourAt(undefined);
    else if (e.key === "Escape" && selected) setSelected(undefined);
    else if ((e.key === "Escape" || e.key === "Backspace") && up) up();
    else return;
    e.preventDefault();
  };

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the tab takes its own keys
    <div className="arch-panel sys-panel" ref={root} tabIndex={0} onKeyDown={onKey}>
      <SystemBar
        crumbs={crumbs}
        level={shown.kind}
        onUp={up}
        onLevel={pickLevel}
        modes={
          shown.kind === "context"
            ? undefined
            : overlay
              ? ["focus", "changed", "all"]
              : ["focus", "all"]
        }
        mode={edgeMode}
        onMode={setMode}
        trace={
          trace && {
            from: trace.from ? crumb(trace.from) : undefined,
            to: trace.to ? crumb(trace.to) : undefined,
            onEnd: (end) => setTrace({ ...trace, [end]: undefined }),
            onClose: () => {
              setTrace(undefined);
              root.current?.focus();
            },
          }
        }
        finding={finding}
        onFind={() => setFinding(true)}
        onTrace={() => {
          setTourAt(undefined);
          setTrace({ from: selected });
        }}
        touring={!!step}
        onTour={startTour}
        change={review ? { on: change, onToggle: () => setChange(!change) } : undefined}
        onDescribe={describe}
      />
      {(error || sendError) && <div className="sys-error error">{sendError ?? error}</div>}
      <div className="sys-work">
        <SystemCanvas
          layout={placed}
          edges={drawnEdges}
          lit={lit}
          hot={hot}
          hotColor={hotColor}
          labels={!focus && !path}
          selected={trace ? undefined : (step?.id ?? sel?.id)}
          stale={stale}
          onSelect={pickNode}
          onOpen={open}
          onReload={reload}
        >
          {shown.kind === "components" && (
            <Minimap
              map={map}
              container={shown.container}
              onPick={(c, drill) =>
                drill
                  ? go({ kind: "components", container: c })
                  : go({ kind: "containers" }, containerId(c))
              }
            />
          )}
          {step && tourAt !== undefined && (
            <TourCard
              steps={steps}
              at={tourAt}
              onAt={setTourAt}
              onClose={() => setTourAt(undefined)}
            />
          )}
        </SystemCanvas>
        {trace ? (
          <TracePanel map={map} colors={colors} hops={hops} onShow={show} onFile={onFile} />
        ) : step && tourAt !== undefined ? (
          <TourSteps steps={steps} at={tourAt} onAt={setTourAt} />
        ) : (
          detail && (
            <SystemDetail
              map={map}
              colors={colors}
              node={detail}
              edges={scene.edges}
              stale={stale.has(detail.id)}
              openable={!!sel || shown.kind === "context"}
              onShow={show}
              onOpen={open}
              onFile={onFile}
              onReveal={(p) => revealInFiles(workspace, p)}
              onAsk={() => sendText(askPrompt(map, detail))}
              onTour={startTour}
            />
          )
        )}
      </div>
      {finding && (
        <FindPalette
          map={map}
          colors={colors}
          onShow={show}
          onFile={onFile}
          onClose={() => {
            setFinding(false);
            root.current?.focus();
          }}
        />
      )}
      {picker}
    </div>
  );
}
