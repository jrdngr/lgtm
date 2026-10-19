import type { ArchDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";

interface Props {
  arch: ArchDto;
  /** The revset, or the commits picked in History. */
  range: string;
  onClearRange?(): void;
  transitive: boolean;
  onTransitive(on: boolean): void;
  /** Open Look items. */
  look: number;
  onAssess(): void;
}

/** Under the bar with the Review lens on: what the review reaches, and Assess. */
export function ReviewStrip(p: Props) {
  const added = p.arch.components.filter((c) => c.status === "added").length;
  const changed = p.arch.components.filter((c) => c.status === "changed").length;
  const { upstream, impacted } = p.arch.counts;
  return (
    <div className="sys-strip-bar">
      <span className="sys-rng">
        {p.range}
        {p.onClearRange && (
          <button type="button" aria-label="Whole Review" onClick={p.onClearRange}>
            ×
          </button>
        )}
      </span>
      <Count n={changed} label="changed" dot="var(--amber)" />
      <Count n={added} label="new" dot="var(--added)" />
      <Count n={upstream} label="upstream" dot="#9fc3e6" />
      <Count n={impacted} label="impacted" dot="imp" />
      <button
        type="button"
        className={`sys-atog${p.transitive ? " on" : ""}`}
        aria-pressed={p.transitive}
        onClick={() => p.onTransitive(!p.transitive)}
      >
        <span className="sys-atk" />
        Transitive
      </button>
      <span className="sys-grow" />
      {p.look > 0 && <Count n={p.look} label="to look at" dot="var(--flagged)" />}
      <PipeButton className="sm" onClick={p.onAssess}>
        Assess
      </PipeButton>
    </div>
  );
}

function Count({ n, label, dot }: { n: number; label: string; dot: string }) {
  return (
    <span className="sys-bk">
      <span
        className={`sys-bd${dot === "imp" ? " imp" : ""}`}
        style={dot === "imp" ? undefined : { background: dot }}
      />
      <b>{n}</b>
      {label}
    </span>
  );
}
