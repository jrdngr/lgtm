import { Panel, useReactFlow } from "@xyflow/react";
import { useEffect, useRef, useState } from "react";
import { IconButton } from "../ui/icons";
import { SectionHeader } from "../ui/SectionHeader";

interface Props {
  changed: number;
  /** Unchanged symbols one hop out, and whether every changed symbol's are shown. */
  callers: number;
  callersOpen: boolean;
  onCallers(open: boolean): void;
  onExpandAll(): void;
  onCollapseAll(): void;
}

/** The graph's toolbar: how many symbols changed, the callers toggle, expand and collapse
 * all, and the legend. */
export function GraphBar(p: Props) {
  const on = p.callers > 0 && p.callersOpen;
  return (
    <SectionHeader title={`${p.changed} changed`}>
      <button
        type="button"
        className={`gchip${on ? " on" : ""}`}
        aria-pressed={on}
        disabled={p.callers === 0}
        title={`${on ? "Hide" : "Show"} ${p.callers} unchanged callers`}
        onClick={() => p.onCallers(!on)}
      >
        Callers
      </button>
      <IconButton icon="expand" label="Expand all" onClick={p.onExpandAll} />
      <IconButton icon="collapse" label="Collapse all" onClick={p.onCollapseAll} />
      <Legend />
    </SectionHeader>
  );
}

/** A ? button opening what the graph's marks mean. Closes on Escape or a click outside. */
function Legend() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
    };
  }, [open]);
  return (
    <div className="menu" ref={ref}>
      <IconButton
        icon="help"
        label="Legend"
        aria-expanded={open}
        className={open ? "open" : undefined}
        onClick={() => setOpen((o) => !o)}
      />
      {open && (
        <dl className="glegend">
          <dt className="gsign-added">+</dt>
          <dd>Added symbol</dd>
          <dt className="gsign-modified">~</dt>
          <dd>Modified symbol</dd>
          <dt className="gsign-removed">−</dt>
          <dd>Removed symbol</dd>
          <dt>
            <span className="gdot risk-high" />
          </dt>
          <dd>Important note, high</dd>
          <dt>
            <span className="gdot risk-medium" />
          </dt>
          <dd>Important note, medium</dd>
          <dt>
            <span className="gdot risk-low" />
          </dt>
          <dd>Important note, low</dd>
          <dt>
            <span className="gline" />
          </dt>
          <dd>Share done</dd>
          <dt>
            <span className="gdash" />
          </dt>
          <dd>Unchanged caller, or a link from one</dd>
          <dt>
            <span className="glit" />
          </dt>
          <dd>Links of the hovered or selected box</dd>
        </dl>
      )}
    </div>
  );
}

/** Zoom in, zoom out and fit, floating at the canvas's bottom right. */
export function ZoomControls() {
  const flow = useReactFlow();
  return (
    <Panel position="bottom-right" className="gzoom">
      <IconButton icon="zoomIn" label="Zoom in" onClick={() => flow.zoomIn()} />
      <IconButton icon="zoomOut" label="Zoom out" onClick={() => flow.zoomOut()} />
      <IconButton icon="fit" label="Fit" onClick={() => flow.fitView({ padding: 0.1 })} />
    </Panel>
  );
}
