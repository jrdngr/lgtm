import { Fragment } from "react";
import { PipeButton } from "../terminal/PipeButton";
import { Arrow, Close, Play, Search, Trace, Up } from "./glyphs";
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
] as const;

const MODES: Record<EdgeMode, string> = { focus: "Focus", changed: "Changed", all: "All" };

interface Props {
  crumbs: Crumb[];
  level: Level["kind"];
  onUp?(): void;
  onLevel(kind: Level["kind"]): void;
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
  finding: boolean;
  onFind(): void;
  onTrace(): void;
  touring: boolean;
  /** Absent when there's no tour. */
  onTour?(): void;
  change?: { on: boolean; onToggle(): void };
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

/** The System tab's toolbar: up a level, where you are, the level, and the tools. */
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
              {c.name}
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
              onClick={() => p.onLevel(kind)}
            >
              <i />
              {label}
            </button>
          </Fragment>
        ))}
      </div>
      <span className="sys-grow" />
      {p.trace ? (
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
          {p.change && (
            <button
              type="button"
              className={`atog${p.change.on ? " on" : ""}`}
              aria-pressed={p.change.on}
              onClick={p.change.onToggle}
            >
              <span className="atrack" />
              Change
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
            className={`sys-tool${p.touring ? " on" : ""}`}
            disabled={!p.onTour}
            onClick={p.onTour}
          >
            <Play />
            Tour
          </button>
          <PipeButton onClick={p.onDescribe}>Describe</PipeButton>
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
