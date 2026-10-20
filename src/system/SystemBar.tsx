import { Fragment, type ReactNode } from "react";
import { Back, Chevron, Close, Forward, Play, Search, Trace } from "./glyphs";
import type { Level } from "./scene";
import { TraceBar, type TraceBarProps } from "./TraceBar";

export interface Crumb {
  id: string;
  name: string;
  /** A container's color; the system's crumb is a white dot. */
  color?: string;
  onClick?(): void;
}

const LEVELS = [
  ["context", "Context"],
  ["containers", "Containers"],
  ["components", "Components"],
  ["symbols", "Symbols"],
] as const;

interface Props {
  crumbs: Crumb[];
  level: Level["kind"];
  onBack?(): void;
  onForward?(): void;
  onLevel(kind: Level["kind"]): void;
  /** Whether there's a component to show the Symbols of. */
  symbols: boolean;
  /** While tracing, its ends replace the tools. */
  trace?: TraceBarProps;
  /** While touring, its name replaces the tools. */
  tour?: { name: string; color: string; onClose(): void };
  finding: boolean;
  onFind(): void;
  onTrace(): void;
  toursOpen: boolean;
  onTours(): void;
  /** Absent with no review open. */
  lens?: { on: boolean; onToggle(): void };
  /** The Generate button. */
  generate: ReactNode;
}

function Swatch({ c }: { c: Crumb }) {
  return (
    <span
      className={`sys-sw${c.color ? "" : " round"}`}
      style={{ background: c.color ?? "#d8d8d8" }}
    />
  );
}

/** The Architecture tab's toolbar: back and forward, where you are, the level, and the tools. */
export function SystemBar(p: Props) {
  const at = LEVELS.findIndex(([k]) => k === p.level);
  return (
    <div className="sys-bar">
      <span className="sys-nav">
        <button
          type="button"
          className="sys-back"
          aria-label="Back"
          disabled={!p.onBack}
          onClick={p.onBack}
        >
          <Back />
        </button>
        <button
          type="button"
          className="sys-back"
          aria-label="Forward"
          disabled={!p.onForward}
          onClick={p.onForward}
        >
          <Forward />
        </button>
      </span>
      <div className="sys-crumbs">
        {p.crumbs.map((c, i) => (
          <Fragment key={c.id}>
            {i > 0 && <span className="sys-sep">›</span>}
            <button
              type="button"
              className={`sys-crumb${i === p.crumbs.length - 1 ? " on" : ""}`}
              onClick={c.onClick}
            >
              {(i === 0 || c.color) && <Swatch c={c} />}
              <span className="sys-cn">{c.name}</span>
            </button>
          </Fragment>
        ))}
      </div>
      <div className="sys-depth">
        {LEVELS.map(([kind, label], i) => (
          <Fragment key={kind}>
            {i > 0 && <span className="sys-step-line" />}
            <button
              type="button"
              className={`sys-step${i === at ? " on" : i < at ? " past" : ""}`}
              aria-pressed={i === at}
              aria-label={label}
              disabled={kind === "symbols" && !p.symbols}
              onClick={() => p.onLevel(kind)}
            >
              <i />
              <span>{label}</span>
            </button>
          </Fragment>
        ))}
      </div>
      <span className="sys-grow" />
      {p.tour ? (
        <span className="sys-tpill" style={{ ["--c" as string]: p.tour.color }}>
          <Play size={12} />
          {p.tour.name} tour
          <button type="button" className="sys-x" aria-label="End Tour" onClick={p.tour.onClose}>
            <Close />
          </button>
        </span>
      ) : p.trace ? (
        <TraceBar {...p.trace} />
      ) : (
        <>
          {p.lens && (
            <button
              type="button"
              className={`sys-lens${p.lens.on ? " on" : ""}`}
              aria-pressed={p.lens.on}
              onClick={p.lens.onToggle}
            >
              <span className="sys-trk" />
              Review
            </button>
          )}
          <button type="button" className={`sys-tool${p.finding ? " on" : ""}`} onClick={p.onFind}>
            <Search />
            Find
            <kbd>/</kbd>
          </button>
          <button type="button" className="sys-tool" onClick={p.onTrace}>
            <Trace />
            Trace
          </button>
          <button
            type="button"
            className={`sys-tool${p.toursOpen ? " on" : ""}`}
            aria-expanded={p.toursOpen}
            data-tours
            onClick={p.onTours}
          >
            <Play />
            Tours
            <span className="sys-down">
              <Chevron size={10} />
            </span>
          </button>
          {p.generate}
        </>
      )}
    </div>
  );
}
