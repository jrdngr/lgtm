import type { SystemDto } from "../bindings";
import { nodeInfo } from "./names";
import type { Hop } from "./trace";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  /** Undefined while an end is unpicked; empty when nothing joins them. */
  hops?: Hop[];
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
}

/** A traced path's hops in order, each with the import site or relation that makes it. */
export function TracePanel({ map, colors, hops, onShow, onFile }: Props) {
  const name = (id: string) => {
    const info = nodeInfo(map, colors, id);
    return (
      <button type="button" className="sys-loc" onClick={() => onShow(id)}>
        <span className="sys-sw" style={{ background: info?.color }} />
        {info?.name ?? id.slice(2)}
      </button>
    );
  };
  return (
    <aside className="sys-detail">
      <div className="sys-steps">
        <h4>Path</h4>
        {hops && !hops.length && <div className="sys-via">No path</div>}
        {hops?.map((h, i) => (
          <div key={`${h.from}>${h.to}`} className="sys-hop">
            <div className="sys-rail">
              <span className="sys-ix">{i + 1}</span>
              {i < hops.length - 1 && <span className="sys-rail-line" />}
            </div>
            <div className="sys-hb">
              <span className="sys-hn">
                {name(h.from)}
                <span className="sys-hn-arrow">→</span>
                {name(h.to)}
              </span>
              {h.path && (
                <button
                  type="button"
                  className="sys-file"
                  onClick={() => h.path && onFile(h.path, h.line)}
                >
                  {h.path}:{h.line}
                </button>
              )}
              {h.label && <span className="sys-via">{h.label}</span>}
            </div>
          </div>
        ))}
      </div>
    </aside>
  );
}
