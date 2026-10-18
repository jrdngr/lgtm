import type { SystemDto } from "../bindings";
import {
  type ContainerInfo,
  componentId,
  containerId,
  containersOf,
  type Level,
  type SEdge,
  type SNode,
} from "./scene";

interface Props {
  map: SystemDto;
  /** The selection, or what the level shows when nothing is selected. */
  node: SNode;
  /** The level's nodes, for naming the other ends of edges. */
  nodes: SNode[];
  edges: SEdge[];
  onPick(level: Level, node: string): void;
  /** Shows a component in the Architecture tab, while the change overlay is on. */
  onArchitecture?(component: string): void;
}

/** Past this many, a list says how many more. */
const LIST = 8;

function Relations({ node, nodes, edges }: Pick<Props, "node" | "nodes" | "edges">) {
  const name = (id: string) => nodes.find((n) => n.id === id)?.name ?? id;
  const mine = edges.filter((e) => e.from === node.id || e.to === node.id);
  if (!mine.length) return null;
  const sorted = [...mine].sort((a, b) => b.imports - a.imports);
  return (
    <section className="sys-sec">
      <h3>Relations</h3>
      {sorted.map((e) => {
        const out = e.from === node.id;
        return (
          <div key={`${e.from}>${e.to}`} className={`sys-row${e.status ? ` ${e.status}` : ""}`}>
            <span className="sys-arrow-glyph">{out ? "→" : "←"}</span>
            <span className="sys-rname">{name(out ? e.to : e.from)}</span>
            <span className="sys-count">{e.label ?? (e.imports || "")}</span>
          </div>
        );
      })}
    </section>
  );
}

function List<T extends { id: string; name: string }>({
  title,
  items,
  count,
  onPick,
}: {
  title: string;
  items: T[];
  count(item: T): number | undefined;
  onPick(item: T): void;
}) {
  if (!items.length) return null;
  return (
    <section className="sys-sec">
      <h3>{title}</h3>
      {items.slice(0, LIST).map((i) => (
        <button key={i.id} type="button" className="sys-row link" onClick={() => onPick(i)}>
          <span className="sys-rname">{i.name}</span>
          <span className="sys-count">{count(i)}</span>
        </button>
      ))}
      {items.length > LIST && <div className="sys-more">{items.length - LIST} more</div>}
    </section>
  );
}

/** The selection's description, what's inside it and its relations. */
export function SystemDetail({ map, node, nodes, edges, onPick, onArchitecture }: Props) {
  const ref = node.id.slice(2);
  const containers = containersOf(map);
  const comps = (c: string) =>
    map.components.filter((k) => k.container === c).sort((a, b) => b.files - a.files);
  const container = (c: ContainerInfo) =>
    c.components
      ? onPick({ kind: "components", container: c.id }, "")
      : onPick({ kind: "containers" }, containerId(c.id));
  return (
    <aside className="sys-detail">
      <header className="sys-head">
        <span className="sys-kind">{node.label}</span>
        <span className="sys-title">{node.name}</span>
        {node.path !== undefined && <span className="sys-path">{node.path || "/"}</span>}
        {node.description && <p>{node.description}</p>}
        {node.meta && <span className="sys-meta">{node.meta}</span>}
        {onArchitecture && node.kind === "component" && (
          <button type="button" className="sys-btn" onClick={() => onArchitecture(ref)}>
            Open in Architecture
          </button>
        )}
      </header>
      {node.kind === "system" && (
        <List
          title="Containers"
          items={containers}
          count={(c) => c.components || undefined}
          onPick={container}
        />
      )}
      {node.kind === "container" && (
        <List
          title="Components"
          items={comps(ref)}
          count={(k) => k.files}
          onPick={(k) => onPick({ kind: "components", container: k.container }, componentId(k.id))}
        />
      )}
      <Relations node={node} nodes={nodes} edges={edges} />
    </aside>
  );
}
