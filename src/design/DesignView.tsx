import { type KeyboardEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import type { DesignItemDto, DesignMapDto, SystemDto } from "../bindings";
import { AgentStrip, EditTools, SelectionBar } from "../canvas/CanvasBars";
import { CanvasOver, CanvasUnder } from "../canvas/CanvasLayers";
import { idsOf } from "../canvas/refs";
import { useCanvas } from "../canvas/useCanvas";
import { useCanvasEdit } from "../canvas/useCanvasEdit";
import { useCanvasFocus } from "../canvas/useCanvasFocus";
import { useWorkspace } from "../core/workspace";
import { openFile } from "../editor/open";
import { useSendText } from "../review/useSendText";
import { useArchView } from "../system/archView";
import { NEUTRAL } from "../system/colors";
import { layout } from "../system/layout";
import { type Crumb, SystemBar } from "../system/SystemBar";
import { SystemCanvas } from "../system/SystemCanvas";
import type { Level } from "../system/scene";
import { PipeButton } from "../terminal/PipeButton";
import { Spinner } from "../ui/Busy";
import type { CtxEntry } from "../ui/contextMenu";
import { DesignDetail } from "./DesignDetail";
import { updateSpecPrompt } from "./designAsk";
import {
  type DesignLevel,
  dcid,
  designNode,
  designScene,
  designSystemId,
  hasDrift,
  openLevel,
  specOf,
} from "./designScene";
import { useSource } from "./designs";
import { useDesignCanvas } from "./useDesignCanvas";
import { useDesignGenerate } from "./useDesignGenerate";
import { levelOfKey, viewKey } from "./viewKeys";
import "../system/system.css";
import "./design.css";
import "../canvas/canvas.css";

interface Props {
  item: DesignItemDto;
  /** The code's map, when there's code. */
  code?: SystemDto;
  /** The Code | Design switch for the bar. */
  source: ReactNode;
}

interface Place {
  level: DesignLevel;
  node?: string;
}

/** Built, planned and differing parts, once code has been compared. */
function counts(m: DesignMapDto) {
  const all = [...m.containers, ...m.components, ...m.externals].filter(
    (n) => n.change !== "removed",
  );
  return {
    built: all.filter((n) => n.built === "built").length,
    planned: all.filter((n) => n.built === "planned").length,
    differs: all.filter((n) => n.built === "differs").length,
  };
}

const NO_HOT = new Map<string, number | undefined>();
const NO_STALE = new Set<string>();

/** A design's map in the Architecture tab, over the code's or alone. */
export function DesignView({ item, code, source }: Props) {
  const workspace = useWorkspace();
  const [error, setError] = useState<string>();
  const { generate, busy, picker: genPicker } = useDesignGenerate(workspace, setError);
  const { sendText, picker } = useSendText(workspace, setError, () =>
    root.current?.querySelector<HTMLElement>(".dsn-keys")?.focus(),
  );
  const root = useRef<HTMLDivElement>(null);
  const m = item.design.map;
  const gen = (
    <PipeButton
      className="sys-btn"
      disabled={busy === item.design.id}
      aria-busy={busy === item.design.id}
      onClick={() => generate(item.design.id)}
    >
      Generate
      {busy === item.design.id && <Spinner />}
    </PipeButton>
  );
  return (
    <div className="sys-panel sys-design" ref={root}>
      {m ? (
        <DesignMap
          key={item.design.id}
          item={item}
          m={m}
          code={code}
          source={source}
          gen={gen}
          onSend={sendText}
        />
      ) : (
        <>
          <SystemBar
            crumbs={[{ id: designSystemId, name: item.design.name }]}
            source={source}
            fourth="Interfaces"
            bare
            level="context"
            onLevel={() => {}}
            symbols={false}
            finding={false}
            onFind={() => {}}
            onTrace={() => {}}
            toursOpen={false}
            onTours={() => {}}
            view={null}
            generate={gen}
          />
          <div className="dsn-empty">Not generated</div>
        </>
      )}
      {error && <div className="sys-error error">{error}</div>}
      {genPicker}
      {picker}
    </div>
  );
}

function DesignMap(p: {
  item: DesignItemDto;
  m: DesignMapDto;
  code?: SystemDto;
  source: ReactNode;
  gen: ReactNode;
  onSend(text: string): void;
}) {
  const { item, m, code } = p;
  const workspace = useWorkspace();
  const { overCode } = useSource();
  const [mapView] = useArchView();
  const [here, setHere] = useState<Place>({ level: { kind: "containers" } });
  const [back, setBack] = useState<Place[]>([]);
  const [forward, setForward] = useState<Place[]>([]);
  const drift = overCode && !!code && hasDrift(m);
  const scene = useMemo(
    () => designScene(m, item.design.name, code, here.level, overCode && !!code),
    [m, item.design.name, code, here.level, overCode],
  );
  const placed = useMemo(() => layout(scene), [scene]);
  const cv = useCanvas(useDesignCanvas(item.design));
  const [editing, setEditing] = useState(false);
  const vkey = viewKey(here.level);
  const ed = useCanvasEdit({
    canvas: cv.canvas,
    commit: cv.commit,
    view: vkey,
    base: placed,
    edges: scene.edges,
    editing,
  });
  const refocus = useCanvasFocus(workspace, {
    kind: "design",
    id: item.design.id,
    view: vkey,
    selection: [...ed.selected],
    parts: ed.arranged.layout.nodes.map((n) => ({
      id: n.id,
      name: n.name,
      x: Math.round(n.x),
      y: Math.round(n.y),
      w: Math.round(n.w),
      h: Math.round(n.h),
    })),
  });
  const [flash, setFlash] = useState<Set<string>>();
  useEffect(() => {
    if (!flash) return;
    const t = setTimeout(() => setFlash(undefined), 1600);
    return () => clearTimeout(t);
  }, [flash]);
  const picked = idsOf(ed.selected, "part");
  const pickedEdges = idsOf(ed.selected, "edge");
  const nodeId = editing ? (picked.length === 1 ? picked[0] : undefined) : here.node;
  const node = nodeId ? designNode(m, code, nodeId) : undefined;

  const go = (level: DesignLevel, id?: string) => {
    if (JSON.stringify(level) !== JSON.stringify(here.level)) {
      setBack([...back, here]);
      setForward([]);
    }
    setHere({ level, node: id });
  };
  const step = (by: -1 | 1) => {
    const from = by < 0 ? back : forward;
    const to = from[from.length - 1];
    if (!to) return;
    if (by < 0) {
      setBack(back.slice(0, -1));
      setForward([...forward, here]);
    } else {
      setForward(forward.slice(0, -1));
      setBack([...back, here]);
    }
    setHere(to);
  };
  const levelOfNode = (id: string): DesignLevel | undefined => {
    if (placed.nodes.some((n) => n.id === id && !n.door)) return here.level;
    if (id.startsWith("k:")) {
      const k = m.components.find((x) => x.id === id.slice(2));
      return k && { kind: "components", container: k.container };
    }
    return { kind: "containers" };
  };
  const show = (id: string) => {
    const at = levelOfNode(id);
    if (at) go(at, id);
  };
  const open = (id: string) => {
    const at = openLevel(m, id);
    if (at) go(at);
  };
  const pick = (id: string | undefined) => {
    const n = placed.nodes.find((x) => x.id === id);
    if (id && n?.door) return openLevel(m, id) ? open(id) : show(id);
    setHere({ ...here, node: id });
  };
  const up = (): (() => void) | undefined => {
    const l = here.level;
    if (l.kind === "interfaces") {
      const k = m.components.find((x) => x.id === l.component);
      return k && (() => go({ kind: "components", container: k.container }, `k:${k.id}`));
    }
    if (l.kind === "components") return () => go({ kind: "containers" }, dcid(l.container));
    if (l.kind === "containers") return () => go({ kind: "context" });
    return undefined;
  };

  const level = here.level;
  const container =
    level.kind === "components"
      ? level.container
      : level.kind === "interfaces"
        ? m.components.find((k) => k.id === level.component)?.container
        : node?.kind === "container"
          ? node.c.id
          : m.containers[0]?.id;
  const component =
    level.kind === "interfaces"
      ? level.component
      : node?.kind === "component"
        ? node.k.id
        : undefined;
  const pickLevel = (kind: Level["kind"]) => {
    if (kind === "context" || kind === "containers") return go({ kind });
    if (kind === "components") return container && go({ kind, container });
    if (component) go({ kind: "interfaces", component });
  };
  const barLevel: Level["kind"] = level.kind === "interfaces" ? "symbols" : level.kind;
  const crumbs: Crumb[] = [
    { id: designSystemId, name: item.design.name, onClick: () => go({ kind: "context" }) },
  ];
  const c = container && m.containers.find((x) => x.id === container);
  if (c && (level.kind === "components" || level.kind === "interfaces"))
    crumbs.push({
      id: dcid(c.id),
      name: c.name,
      onClick: () => go({ kind: "components", container: c.id }),
    });
  const k = component && m.components.find((x) => x.id === component);
  if (k && level.kind === "interfaces") crumbs.push({ id: `k:${k.id}`, name: k.name });

  const onFile = (path: string, line?: number) => openFile(workspace, path, line);
  const nodeMenu = (id: string): CtxEntry[] => {
    const n = designNode(m, code, id);
    const spec = specOf(n)[0];
    const down = openLevel(m, id);
    return [
      down && {
        label:
          down.kind === "interfaces"
            ? "Interfaces"
            : down.kind === "components"
              ? "Components"
              : "Containers",
        onSelect: () => open(id),
      },
      spec && { label: "Open Spec", onSelect: () => onFile(spec.path, spec.line) },
    ];
  };
  const showAgent = () => {
    const pass = cv.canvas.agent;
    if (!pass) return;
    if (pass.view !== vkey) go(levelOfKey(pass.view));
    setFlash(new Set(pass.touched));
  };
  const onKey = (e: KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    if (editing && ed.onKey(e)) e.preventDefault();
    else if (editing && mod && e.key.toLowerCase() === "z") {
      if (e.shiftKey) cv.redo();
      else cv.undo();
      e.preventDefault();
    } else if (!mod && !e.altKey && e.key === "e") {
      setEditing(!editing);
      e.preventDefault();
    } else if (editing && e.key === "Escape") {
      setEditing(false);
      e.preventDefault();
    } else onNavKey(e);
  };
  const onNavKey = (e: KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey;
    if (e.ctrlKey && !e.metaKey && (e.key === "-" || e.key === "_")) step(e.shiftKey ? 1 : -1);
    else if (mod) return;
    else if (e.key === "Escape" && here.node) setHere({ ...here, node: undefined });
    else if ((e.key === "Escape" || e.key === "Backspace") && up()) up()?.();
    else return;
    e.preventDefault();
  };
  const n = counts(m);
  const legend = overCode && !!code && !drift && level.kind !== "context";

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the tab takes its own keys
    <div className="dsn-keys" tabIndex={0} onKeyDown={onKey} onFocus={refocus}>
      <SystemBar
        crumbs={crumbs}
        source={p.source}
        fourth="Interfaces"
        bare
        level={barLevel}
        onBack={back.length ? () => step(-1) : undefined}
        onForward={forward.length ? () => step(1) : undefined}
        onLevel={pickLevel}
        symbols={!!component}
        finding={false}
        onFind={() => {}}
        onTrace={() => {}}
        toursOpen={false}
        onTours={() => {}}
        view={
          <>
            {drift && (
              <span className="dsn-counts">
                <span className="built">✓ {n.built} built</span>·
                <span className="planned">{n.planned} planned</span>·
                <span className="differs">{n.differs} differs</span>
              </span>
            )}
            <button
              type="button"
              className={`sys-tool${editing ? " on" : ""}`}
              aria-pressed={editing}
              onClick={() => setEditing(!editing)}
            >
              <svg
                width="13"
                height="13"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                aria-hidden="true"
              >
                <path d="M11 2.5l2.5 2.5L6 12.5H3.5V10z" />
              </svg>
              Edit
              <kbd>e</kbd>
            </button>
          </>
        }
        generate={p.gen}
      />
      {cv.canvas.agent && (
        <AgentStrip pass={cv.canvas.agent} onUndo={cv.undoAgent} onShow={showAgent} />
      )}
      {cv.error && <div className="sys-error error">{cv.error}</div>}
      <div className="sys-work">
        <SystemCanvas
          layout={ed.arranged.layout}
          edges={scene.edges}
          labels={mapView.labels}
          hot={NO_HOT}
          hotColor={NEUTRAL}
          selected={editing ? undefined : here.node}
          selectedEdge={pickedEdges.length === 1 ? pickedEdges[0] : undefined}
          focus={
            !editing && placed.nodes.some((n) => n.id === here.node && !n.door)
              ? here.node
              : undefined
          }
          stale={NO_STALE}
          onSelect={editing ? () => {} : pick}
          onOpen={open}
          onEdge={(e) => editing && ed.selectEdge(`${e.from}>${e.to}`, false)}
          nodeMenu={nodeMenu}
          onReload={() => {}}
          quiet
          paints={ed.paints}
          flash={flash}
          fitKey={`${item.design.id}:${vkey}`}
          under={<CanvasUnder ed={ed} editing={editing} />}
          over={<CanvasOver ed={ed} editing={editing} flash={flash} />}
          screen={(toScreen, toWorld) => {
            ed.setToWorld(toWorld);
            return (
              editing && (
                <>
                  <SelectionBar ed={ed} canvas={cv.canvas} commit={cv.commit} toScreen={toScreen} />
                  <EditTools ed={ed} onResetLayout={ed.resetLayout} />
                </>
              )
            );
          }}
          edit={
            editing
              ? {
                  selected: ed.selected,
                  fresh: ed.arranged.fresh,
                  panning: ed.panning,
                  onNodeDown: ed.onNodeDown,
                  onBackgroundDown: ed.onBackgroundDown,
                }
              : undefined
          }
        >
          {legend && (
            <div className="dsn-legend">
              <span>
                <i className="new" />
                New
              </span>
              <span>
                <i className="changed" />
                Changes
              </span>
              <span>
                <i className="removed" />
                Removed
              </span>
              <span>
                <i />
                Unchanged
              </span>
            </div>
          )}
        </SystemCanvas>
        <DesignDetail
          m={m}
          node={node}
          drift={drift}
          overCode={overCode && !!code}
          onShow={show}
          onFile={onFile}
          onUpdateSpec={() => node && p.onSend(updateSpecPrompt(item.design, m, node))}
        />
      </div>
    </div>
  );
}
