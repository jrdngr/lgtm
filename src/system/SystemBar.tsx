import { Fragment, type ReactNode } from "react";
import { Back, Chevron, Close, Forward, Play, Search, Trace } from "./glyphs";
import type { Level } from "./scene";
import { TraceBar, type TraceBarProps } from "./TraceBar";

export interface Crumb {
  id: string;
  name: string;
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
  /** Code or a design, after the crumbs. */
  source?: ReactNode;
  /** Names the fourth level, Symbols by default. */
  fourth?: string;
  /** Hides Review, Find, Trace and Tours. */
  bare?: boolean;
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
  /** The View button. */
  view: ReactNode;
  /** The Generate button. */
  generate: ReactNode;
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
              <span className="sys-cn">{c.name}</span>
            </button>
          </Fragment>
        ))}
      </div>
      {p.source}
      <div className="sys-levels">
        {LEVELS.map(([kind, name], i) => {
          const label = kind === "symbols" && p.fourth ? p.fourth : name;
          return (
            <button
              key={kind}
              type="button"
              className={i === at ? "on" : undefined}
              aria-pressed={i === at}
              disabled={kind === "symbols" && !p.symbols}
              onClick={() => p.onLevel(kind)}
            >
              {label}
            </button>
          );
        })}
      </div>
      <div className="sys-tools">
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
        ) : p.bare ? (
          <>
            {p.view}
            {p.generate}
          </>
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
            <button
              type="button"
              className={`sys-tool${p.finding ? " on" : ""}`}
              onClick={p.onFind}
            >
              <Search />
              Find
              <kbd>/</kbd>
            </button>
            <button type="button" className="sys-tool" onClick={p.onTrace}>
              <Trace />
              Trace
            </button>
            {p.view}
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
    </div>
  );
}
