import { Fragment } from "react";
import type { SystemDto } from "../bindings";
import { nodeInfo } from "./names";
import { containerId, containerOf } from "./scene";
import type { Hop } from "./trace";
import type { TraceShow } from "./useTrace";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  from: string;
  to: string;
  /** The shortest path; empty when one end holds the other or nothing joins them. */
  hops: Hop[];
  /** Every path, fewest hops first; empty when nothing joins the ends. */
  all: Hop[][];
  show: TraceShow;
  onMode(s: TraceShow): void;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
}

type Item =
  | { kind: "node"; id: string }
  | { kind: "hop"; hop: Hop; n: number; back: boolean }
  /** Between a container and a component in it. */
  | { kind: "in" };

/** Whether `a` is `b`, or one holds the other. */
function near(map: SystemDto, a: string, b: string) {
  const holds = (c: string, k: string) =>
    c.startsWith("c:") && k.startsWith("k:") && containerId(containerOf(map, k.slice(2))) === c;
  return a === b || holds(a, b) || holds(b, a);
}

/** A path's nodes from `from` to `to` in order, with the hops between them, each pointing the
 * way its import or relation does. */
function chain(map: SystemDto, from: string, to: string, hops: Hop[]): Item[] {
  const out: Item[] = [{ kind: "node", id: from }];
  let at = from;
  const enter = (id: string) => {
    if (id === at) return;
    out.push({ kind: "in" }, { kind: "node", id });
    at = id;
  };
  hops.forEach((hop, i) => {
    const back = !near(map, hop.from, at) && near(map, hop.to, at);
    enter(back ? hop.to : hop.from);
    out.push({ kind: "hop", hop, n: i + 1, back });
    at = back ? hop.from : hop.to;
    out.push({ kind: "node", id: at });
  });
  enter(to);
  return out;
}

const short = (path: string) => path.split("/").slice(-2).join("/");

/** Shortest, All paths, or Show on map. */
export function TraceModes({
  show,
  count,
  onMode,
}: {
  show: TraceShow;
  count: number;
  onMode(s: TraceShow): void;
}) {
  const modes: [TraceShow, string][] = [
    ["shortest", "Shortest"],
    ["all", `All paths ${count}`],
    ["map", "Show on map"],
  ];
  return (
    <div className="sys-tmodes">
      {modes.map(([m, label]) => (
        <button
          key={m}
          type="button"
          className={`sys-tool${show === m ? " on" : ""}`}
          aria-pressed={show === m}
          onClick={() => onMode(m)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/** A traced path drawn straight, left to right, or every path a few hops longer, one a row. */
export function TraceView(p: Props) {
  const node = (id: string) => {
    const info = nodeInfo(p.map, p.colors, id);
    const where =
      info?.kind === "component" && info.container !== undefined
        ? nodeInfo(p.map, p.colors, containerId(info.container))?.name
        : info?.kind;
    const round = info?.kind === "person" || info?.kind === "external";
    return (
      <button
        type="button"
        className={`sys-pn${id === p.from || id === p.to ? " end" : ""}`}
        onClick={() => p.onShow(id)}
      >
        <b>
          <span className={`sys-sw${round ? " round" : ""}`} style={{ background: info?.color }} />
          <span className="sys-cn">{info?.name ?? id.slice(2)}</span>
        </b>
        {where && <small>{where}</small>}
      </button>
    );
  };
  const row = (hops: Hop[]) =>
    chain(p.map, p.from, p.to, hops).map((it, i) => (
      // biome-ignore lint/suspicious/noArrayIndexKey: a row's items never reorder
      <Fragment key={i}>
        {it.kind === "node" ? (
          node(it.id)
        ) : it.kind === "in" ? (
          <span className="sys-pin" />
        ) : (
          <span className={`sys-phop${it.back ? " back" : ""}`}>
            <span className="sys-pnum">{it.n}</span>
            <span className="sys-pline" />
            {it.hop.path ? (
              <button
                type="button"
                className="sys-psite"
                onClick={() => it.hop.path && p.onFile(it.hop.path, it.hop.line)}
              >
                {short(it.hop.path)}:{it.hop.line}
              </button>
            ) : (
              <span className="sys-psite">{it.hop.label}</span>
            )}
          </span>
        )}
      </Fragment>
    ));
  const rows = p.show === "all" ? p.all : [p.hops];
  return (
    <div className="sys-tview">
      <div className="sys-tscroll">
        <div className="sys-prows">
          {p.all.length ? (
            rows.map((hops) => (
              <div key={hops.map((h) => `${h.from}>${h.to}`).join(" ")} className="sys-prow">
                {row(hops)}
              </div>
            ))
          ) : (
            <div className="sys-via">No path</div>
          )}
        </div>
      </div>
      <TraceModes show={p.show} count={p.all.length} onMode={p.onMode} />
    </div>
  );
}
