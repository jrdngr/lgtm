import type { SystemDto } from "../bindings";
import { Close } from "./glyphs";
import { nodeInfo } from "./names";
import { componentId, containerId, SYSTEM } from "./scene";

export interface Step {
  key: string;
  /** The node it shows. */
  id: string;
  name: string;
  color: string;
  kind: string;
  text: string;
}

const KINDS: Record<string, string> = {
  system: "Context",
  container: "Container",
  component: "Component",
};

/** The description's tour, with each step's node. Steps on nodes the map lost are skipped. */
export function tourSteps(map: SystemDto, colors: Map<string, string>): Step[] {
  return (map.description?.tour ?? []).flatMap((t, i) => {
    const ids = t.node === "system" ? [SYSTEM] : [containerId(t.node), componentId(t.node)];
    for (const id of ids) {
      const info = nodeInfo(map, colors, id);
      if (info)
        return [
          {
            key: `${i}:${id}`,
            id,
            name: info.name,
            color: info.color,
            kind: KINDS[info.kind] ?? "",
            text: t.text,
          },
        ];
    }
    return [];
  });
}

interface Props {
  steps: Step[];
  at: number;
  onAt(i: number): void;
  onClose(): void;
}

/** The tour's current step, over the canvas. */
export function TourCard({ steps, at, onAt, onClose }: Props) {
  const step = steps[at];
  if (!step) return null;
  return (
    <div
      className="sys-tour"
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <div className="sys-tour-row">
        <span className="sys-tour-step">
          {at + 1} / {steps.length}
        </span>
        <span className="sys-sw" style={{ background: step.color }} />
        <span className="sys-tour-name">{step.name}</span>
        <span className="sys-grow" />
        <button type="button" className="sys-x" aria-label="End Tour" onClick={onClose}>
          <Close />
        </button>
      </div>
      <p>{step.text}</p>
      <div className="sys-tour-row">
        <div className="sys-dots">
          {steps.map((s, i) => (
            <span
              key={s.key}
              className={i === at ? "on" : undefined}
              style={i === at ? { background: step.color } : undefined}
            />
          ))}
        </div>
        <span className="sys-grow" />
        <button type="button" className="sys-btn" disabled={at === 0} onClick={() => onAt(at - 1)}>
          Back
        </button>
        {at < steps.length - 1 ? (
          <button type="button" className="sys-btn pri" onClick={() => onAt(at + 1)}>
            Next
          </button>
        ) : (
          <button type="button" className="sys-btn pri" onClick={onClose}>
            Done
          </button>
        )}
      </div>
    </div>
  );
}

/** The tour's steps, in the side panel. */
export function TourSteps({ steps, at, onAt }: Omit<Props, "onClose">) {
  return (
    <aside className="sys-detail">
      <div className="sys-steps">
        <h4>Tour</h4>
        {steps.map((s, i) => (
          <button
            key={s.key}
            type="button"
            className={`sys-ts${i === at ? " on" : i < at ? " done" : ""}`}
            onClick={() => onAt(i)}
          >
            <span
              className="sys-ix"
              style={i === at ? { background: s.color, color: "#16171a" } : undefined}
            >
              {i + 1}
            </span>
            {s.name}
            <span className="sys-lv">{s.kind}</span>
          </button>
        ))}
      </div>
    </aside>
  );
}
