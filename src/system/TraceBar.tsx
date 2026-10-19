import { Close, Swap } from "./glyphs";
import { TraceField } from "./TraceField";
import type { Target } from "./traceTargets";
import type { End } from "./useTrace";
import "./trace.css";

export interface TraceBarProps {
  from?: string;
  to?: string;
  active?: End;
  targets: Target[];
  onEdit(end: End): void;
  onPick(end: End, id: string): void;
  onSwap(): void;
  onClose(): void;
}

/** A trace's ends, which replace the bar's tools while tracing. */
export function TraceBar(p: TraceBarProps) {
  const field = (end: End, label: string) => (
    <TraceField
      label={label}
      value={p.targets.find((t) => t.id === p[end])}
      editing={p.active === end}
      targets={p.targets}
      skip={p[end === "from" ? "to" : "from"]}
      onEdit={() => p.onEdit(end)}
      onPick={(id) => p.onPick(end, id)}
    />
  );
  return (
    <>
      {field("from", "From")}
      <button type="button" className="sys-tool sys-tswap" aria-label="Swap" onClick={p.onSwap}>
        <Swap />
      </button>
      {field("to", "To")}
      <button
        type="button"
        className="sys-tool sys-tswap"
        aria-label="End Trace"
        onClick={p.onClose}
      >
        <Close />
      </button>
    </>
  );
}
