import type { IDockviewPanelProps } from "dockview-react";
import { type KeyboardEvent, useEffect, useMemo, useState } from "react";
import type { DrawingDto } from "../bindings";
import { AgentStrip, EditTools, SelectionBar } from "../canvas/CanvasBars";
import { CanvasOver, CanvasUnder } from "../canvas/CanvasLayers";
import { type CanvasStore, useCanvas } from "../canvas/useCanvas";
import { useCanvasEdit } from "../canvas/useCanvasEdit";
import { useCanvasFocus } from "../canvas/useCanvasFocus";
import { useWorkspace } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { NEUTRAL } from "../system/colors";
import type { Layout } from "../system/layout";
import { SystemCanvas } from "../system/SystemCanvas";
import { Menu } from "../ui/Menu";
import { copyPng, drawingSvg, saveAs, svgToPng } from "./exportDrawing";
import { useDrawing } from "./store";
import "../system/system.css";
import "../canvas/canvas.css";
import "./drawings.css";

const VIEW = "main";
const EMPTY: Layout = { nodes: [], tiers: [], width: 0, height: 0 };
const NO_HOT = new Map<string, number | undefined>();
const NO_STALE = new Set<string>();

/** A drawing in its own tab. */
export function DrawingTab({ api, params }: IDockviewPanelProps<{ drawing: string }>) {
  const { drawing, error } = useDrawing(params.drawing);
  useEffect(() => {
    if (drawing) api.setTitle(drawing.name);
  }, [api, drawing]);
  if (!drawing)
    return (
      <div className="sys-panel dr-tab">
        {error && <div className="sys-error error">{error}</div>}
      </div>
    );
  return <Drawing key={drawing.id} drawing={drawing} />;
}

function useDrawingStore(d: DrawingDto): CanvasStore {
  const { id, canvas } = d;
  return useMemo(
    () => ({
      stored: canvas,
      save: (c) => unwrap(commands.saveDrawingCanvas(id, c)),
      undoAgent: () => unwrap(commands.undoDrawingAgent(id)).then((x) => x.canvas),
    }),
    [id, canvas],
  );
}

function Drawing({ drawing }: { drawing: DrawingDto }) {
  const workspace = useWorkspace();
  const cv = useCanvas(useDrawingStore(drawing));
  const ed = useCanvasEdit({
    canvas: cv.canvas,
    commit: cv.commit,
    view: VIEW,
    base: EMPTY,
    edges: [],
    editing: true,
  });
  const [flash, setFlash] = useState<Set<string>>();
  const [error, setError] = useState<string>();
  useEffect(() => {
    if (!flash) return;
    const t = setTimeout(() => setFlash(undefined), 1600);
    return () => clearTimeout(t);
  }, [flash]);
  const refocus = useCanvasFocus(workspace, {
    kind: "drawing",
    id: drawing.id,
    view: VIEW,
    selection: [...ed.selected],
    parts: [],
  });

  const name = drawing.name;
  const run = (f: () => Promise<unknown>) =>
    f().then(
      () => setError(undefined),
      (e) => setError(errorText(e)),
    );
  const exports = [
    { label: "SVG…", onSelect: () => run(() => saveAs(name, "svg", drawingSvg(ed))) },
    {
      label: "PNG…",
      onSelect: () => run(async () => saveAs(name, "png", await svgToPng(drawingSvg(ed)))),
    },
    {
      label: "JSON…",
      onSelect: () => run(() => saveAs(name, "json", JSON.stringify(ed.view, null, 2))),
    },
    { label: "Copy as PNG", onSelect: () => run(() => copyPng(svgToPng(drawingSvg(ed)))) },
  ];

  const onKey = (e: KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    const mod = e.metaKey || e.ctrlKey;
    if (ed.onKey(e)) e.preventDefault();
    else if (mod && e.key.toLowerCase() === "z") {
      if (e.shiftKey) cv.redo();
      else cv.undo();
      e.preventDefault();
    }
  };

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the tab takes its own keys
    <div className="sys-panel dr-tab" tabIndex={0} onKeyDown={onKey} onFocus={refocus}>
      <div className="dr-bar">
        <b>{name}</b>
        <span className="spacer" />
        <Menu text="Export" label="Export" items={exports} />
      </div>
      {cv.canvas.agent && (
        <AgentStrip
          pass={cv.canvas.agent}
          onUndo={cv.undoAgent}
          onShow={() => setFlash(new Set(cv.canvas.agent?.touched))}
        />
      )}
      {(cv.error || error) && <div className="sys-error error">{cv.error || error}</div>}
      <div className="sys-work">
        <SystemCanvas
          layout={ed.arranged.layout}
          edges={[]}
          labels="off"
          hot={NO_HOT}
          hotColor={NEUTRAL}
          stale={NO_STALE}
          onSelect={() => {}}
          onOpen={() => {}}
          onEdge={() => {}}
          nodeMenu={() => []}
          onReload={() => {}}
          quiet
          flash={flash}
          fitKey={drawing.id}
          under={<CanvasUnder ed={ed} editing />}
          over={<CanvasOver ed={ed} editing flash={flash} />}
          screen={(toScreen, toWorld) => {
            ed.setToWorld(toWorld);
            return (
              <>
                <SelectionBar ed={ed} canvas={cv.canvas} commit={cv.commit} toScreen={toScreen} />
                <EditTools ed={ed} />
              </>
            );
          }}
          edit={{
            selected: ed.selected,
            fresh: ed.arranged.fresh,
            panning: ed.panning,
            onNodeDown: ed.onNodeDown,
            onBackgroundDown: ed.onBackgroundDown,
          }}
        />
      </div>
    </div>
  );
}
