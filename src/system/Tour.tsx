import type { Stop } from "./tours";

interface Props {
  stops: Stop[];
  at: number;
  onAt(i: number): void;
  onClose(): void;
  /** Opens the step's file or symbol. */
  onOpen?(): void;
}

const where = (s: Stop) =>
  s.symbol
    ? `${s.symbol.path.slice(s.symbol.path.lastIndexOf("/") + 1)}${s.symbol.line ? `:${s.symbol.line}` : ""}`
    : undefined;

/** The tour's current step, over the canvas. */
export function TourCard({ stops, at, onAt, onClose, onOpen }: Props) {
  const stop = stops[at];
  if (!stop) return null;
  return (
    <div
      className="sys-tour"
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <div className="sys-tour-row">
        <span className="sys-tour-step">
          {at + 1} / {stops.length}
        </span>
        {!stop.mono && <span className="sys-sw" style={{ background: stop.color }} />}
        <span className={`sys-tour-name${stop.mono ? " mono" : ""}`}>{stop.name}</span>
        {where(stop) && <span className="sys-tour-at">{where(stop)}</span>}
        <span className="sys-grow" />
        {onOpen && (
          <button type="button" className="sys-btn sm" onClick={onOpen}>
            Open
          </button>
        )}
      </div>
      <p>{stop.text}</p>
      <div className="sys-tour-row">
        <div className="sys-dots">
          {stops.map((s, i) => (
            <span key={s.key} className={i === at ? "on" : i < at ? "done" : undefined} />
          ))}
        </div>
        <span className="sys-grow" />
        <button type="button" className="sys-btn" disabled={at === 0} onClick={() => onAt(at - 1)}>
          Back
        </button>
        {at < stops.length - 1 ? (
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

/** The tour's steps in the side panel, under how deep the current one is. */
export function TourSteps({
  title,
  color,
  stops,
  at,
  onAt,
}: Omit<Props, "onClose"> & { title: string; color: string }) {
  const depth = stops[at]?.depth ?? 0;
  return (
    <aside className="sys-detail">
      <div className="sys-th">
        <span className="sys-tt">
          <span className="sys-sw" style={{ background: color }} />
          {title}
        </span>
      </div>
      <div className="sys-lvbar">
        {[0, 1, 2, 3].map((d) => (
          <span key={d} className={d === depth ? "on" : d < depth ? "d" : undefined} />
        ))}
      </div>
      <div className="sys-tsteps">
        {stops.map((s, i) => (
          <button
            key={s.key}
            type="button"
            className={`sys-ts${i === at ? " on" : i < at ? " done" : ""}`}
            onClick={() => onAt(i)}
          >
            <span className="sys-ix">{i + 1}</span>
            <span className="sys-tw">
              <span className={`sys-tn${s.mono ? " mono" : ""}`}>{s.name}</span>
              <span className="sys-lv">{s.kind}</span>
            </span>
          </button>
        ))}
      </div>
    </aside>
  );
}
