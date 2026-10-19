import { Fragment } from "react";
import { PipeButton } from "../terminal/PipeButton";
import { Arrow, Chevron, Close, Play, Search, Trace, Up } from "./glyphs";
import type { Level } from "./scene";

export interface Crumb {
  id: string;
  name: string;
  /** A container's color; the system's crumb is a white dot. */
  color?: string;
  onClick?(): void;
}

export type EdgeMode = "focus" | "changed" | "all";

const LEVELS = [
  ["context", "Context"],
  ["containers", "Containers"],
  ["components", "Components"],
  ["symbols", "Symbols"],
] as const;

const MODES: Record<EdgeMode, string> = { focus: "Focus", changed: "Changed", all: "All" };

interface Props {
  crumbs: Crumb[];
  level: Level["kind"];
  onUp?(): void;
  onLevel(kind: Level["kind"]): void;
  /** Whether there's a component to show the Symbols of. */
  symbols: boolean;
  /** Edge modes this level offers, if any. */
  modes?: EdgeMode[];
  mode: EdgeMode;
  onMode(m: EdgeMode): void;
  /** While tracing, its ends replace the tools. */
  trace?: {
    from?: Crumb;
    to?: Crumb;
    onEnd(end: "from" | "to"): void;
    onClose(): void;
  };
  /** While touring, its name replaces the tools. */
  tour?: { name: string; color: string; onClose(): void };
  finding: boolean;
  onFind(): void;
  onTrace(): void;
  toursOpen: boolean;
  onTours(): void;
  /** Absent with no review open. */
  lens?: { on: boolean; onToggle(): void };
  onDescribe(): void;
}

function Swatch({ c }: { c: Crumb }) {
  return (
    <span
      className={`sys-sw${c.color ? "" : " round"}`}
      style={{ background: c.color ?? "#d8d8d8" }}
    />
  );
}

/** The Architecture tab's toolbar: up a level, where you are, the level, and the tools. */
export function SystemBar(p: Props) {
  const at = LEVELS.findIndex(([k]) => k === p.level);
  return (
    <div className="sys-bar">
      <button
        type="button"
        className="sys-back"
        aria-label="Up a Level"
        disabled={!p.onUp}
        onClick={p.onUp}
      >
        <Up />
      </button>
      <div className="sys-crumbs">
        {p.crumbs.map((c, i) => (
          <Fragment key={c.id}>
            {i > 0 && <span className="sys-sep">›</span>}
            <button
              type="button"
              className={`sys-crumb${i === p.crumbs.length - 1 ? " on" : ""}`}
              onClick={c.onClick}
            >
              {i < 2 && <Swatch c={c} />}
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
        <div className="sys-trace">
          Trace
          <TraceEnd c={p.trace.from} onClick={() => p.trace?.onEnd("from")} />
          <Arrow />
          <TraceEnd c={p.trace.to} onClick={() => p.trace?.onEnd("to")} />
          <button type="button" className="sys-x" aria-label="End Trace" onClick={p.trace.onClose}>
            <Close />
          </button>
        </div>
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
          {p.modes && (
            <span className="sys-seg">
              {p.modes.map((m) => (
                <button
                  key={m}
                  type="button"
                  className={p.mode === m ? "on" : undefined}
                  aria-pressed={p.mode === m}
                  onClick={() => p.onMode(m)}
                >
                  {MODES[m]}
                </button>
              ))}
            </span>
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
          {!p.lens?.on && <PipeButton onClick={p.onDescribe}>Describe</PipeButton>}
        </>
      )}
    </div>
  );
}

function TraceEnd({ c, onClick }: { c?: Crumb; onClick(): void }) {
  return (
    <button type="button" className={`sys-end${c ? "" : " empty"}`} onClick={onClick}>
      {c && <Swatch c={c} />}
      {c?.name ?? "…"}
    </button>
  );
}
