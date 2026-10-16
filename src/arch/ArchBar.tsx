import type { ArchRange, CountsDto } from "../bindings";
import type { Mode } from "./scene";

export interface Crumb {
  label: string;
  /** Goes back to this level. The last crumb, where the graph is, has none. */
  onClick?(): void;
}

interface Props {
  crumbs: Crumb[];
  /** Absent in the drill-down, which has no Changed/All. */
  mode?: Mode;
  onMode(mode: Mode): void;
  range?: ArchRange;
  onClearRange(): void;
  counts: CountsDto;
  transitive: boolean;
  onTransitive(on: boolean): void;
}

const short = (id: string) => id.slice(0, 8);

/** The tab's bar: where the graph is, Changed/All, the commits picked in History, blast radius
 * counts and the Transitive toggle. */
export function ArchBar(p: Props) {
  return (
    <div className="abar">
      {p.crumbs.length > 0 && (
        <span className="acrumbs">
          {p.crumbs.map((c, i) => (
            // A container and its loose files' component share a name.
            // biome-ignore lint/suspicious/noArrayIndexKey: crumbs are a fixed path
            <span key={i} className="acrumb">
              {i > 0 && <i>›</i>}
              {c.onClick ? (
                <button type="button" className="alink" onClick={c.onClick}>
                  {c.label}
                </button>
              ) : (
                c.label
              )}
            </span>
          ))}
        </span>
      )}
      {p.mode && (
        <span className="aseg">
          {(["changed", "all"] as const).map((m) => (
            <button
              key={m}
              type="button"
              className={p.mode === m ? "on" : undefined}
              aria-pressed={p.mode === m}
              onClick={() => p.onMode(m)}
            >
              {m === "changed" ? "Changed" : "All"}
            </button>
          ))}
        </span>
      )}
      {p.range && (
        <button type="button" className="apill gray" onClick={p.onClearRange}>
          {p.range.from ? `${short(p.range.from)}..${short(p.range.to)}` : short(p.range.to)}
        </button>
      )}
      <span className="acounts">
        <span>
          <b>{p.counts.changed}</b> changed
        </span>
        <span>
          <b>{p.counts.upstream}</b> upstream
        </span>
        <span className="imp">
          <b>{p.counts.impacted}</b> impacted
        </span>
      </span>
      <button
        type="button"
        className={`atog${p.transitive ? " on" : ""}`}
        aria-pressed={p.transitive}
        onClick={() => p.onTransitive(!p.transitive)}
      >
        <span className="atrack" />
        Transitive
      </button>
    </div>
  );
}
