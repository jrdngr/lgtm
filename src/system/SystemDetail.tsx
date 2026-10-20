import type { LookDto, SystemDto, TourDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";
import { tint } from "./colors";
import { Chevron, Play } from "./glyphs";
import { groupsOf } from "./groups";
import { ChangedFiles, ChangedInside, LookList } from "./LensSections";
import { isChanged, type Lens } from "./lens";
import { MostUsed } from "./MostUsed";
import { nodeInfo } from "./names";
import { componentId, containerId, containersOf, type SEdge, type SNode, SYSTEM } from "./scene";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  /** The selection, or what the level shows when nothing is selected. */
  node: SNode;
  /** The level's edges, for what the node talks to. */
  edges: SEdge[];
  stale: boolean;
  /** False when the node is what the level already shows inside. */
  openable: boolean;
  /** Shows a node, at the level that holds it. */
  onShow(id: string): void;
  /** Opens a container, or the system, a level down. */
  onOpen(id: string): void;
  onFile(path: string, line?: number): void;
  onReveal(path: string): void;
  onAsk(): void;
  /** The node's tour, or the system's overview. */
  tour?: TourDto;
  onTour(tour: TourDto): void;
  /** Shows a component's symbols. */
  onSymbols(id: string): void;
  /** With the Review lens on. */
  lens?: Lens;
  onLook(l: LookDto): void;
  /** Shows a changed file in the review. */
  onChangedFile(path: string): void;
}

/** Past this many, a list says how many more. */
const LIST = 5;

function Swatch({ color, dashed }: { color: string; dashed?: boolean }) {
  return dashed ? (
    <span className="sys-sw dashed" />
  ) : (
    <span className="sys-sw" style={{ background: color }} />
  );
}

function TalksTo({ map, colors, node, edges, onShow }: Props) {
  const mine = edges.filter((e) => e.from === node.id || e.to === node.id);
  if (!mine.length) return null;
  const sorted = [...mine].sort(
    (a, b) => Number(b.from === node.id) - Number(a.from === node.id) || b.imports - a.imports,
  );
  return (
    <section className="sys-sec">
      <h4>Talks to</h4>
      {sorted.map((e) => {
        const out = e.from === node.id;
        const other = out ? e.to : e.from;
        const info = nodeInfo(map, colors, other);
        if (!info) return null;
        return (
          <button
            key={`${e.from}>${e.to}`}
            type="button"
            className={`sys-li${e.status ? ` ${e.status}` : ""}`}
            onClick={() => onShow(other)}
          >
            <span className="sys-arr">{out ? "→" : "←"}</span>
            <Swatch color={info.color} dashed={info.kind === "external"} />
            <span>{info.name}</span>
            {(e.label || e.status) && (
              <span className="sys-lbl">
                {e.status === "added"
                  ? `new${e.label ? ` · ${e.label}` : ""}`
                  : e.status === "removed"
                    ? "removed"
                    : e.label}
              </span>
            )}
          </button>
        );
      })}
    </section>
  );
}

function SystemBody({ map, colors, onShow }: Props) {
  const containers = containersOf(map).filter((c) => c.id !== "");
  const most = Math.max(1, ...containers.map((c) => c.files));
  const around = [
    ...(map.description?.people ?? []).map((p) => `p:${p.name}`),
    ...(map.description?.externals ?? []).map((x) => `x:${x.name}`),
  ];
  return (
    <>
      <section className="sys-sec">
        <h4>Containers</h4>
        {containers.map((c) => {
          const color = colors.get(c.id) ?? "#8b8d93";
          return (
            <button
              key={c.id}
              type="button"
              className="sys-li"
              onClick={() => onShow(containerId(c.id))}
            >
              <Swatch color={color} />
              <span>{c.name}</span>
              <span
                className="sys-bar2"
                style={{ width: Math.max(4, (100 * c.files) / most), background: color }}
              />
            </button>
          );
        })}
      </section>
      {!!around.length && (
        <section className="sys-sec">
          <h4>Around it</h4>
          {around.map((id) => (
            <button key={id} type="button" className="sys-li" onClick={() => onShow(id)}>
              {id.slice(2)}
            </button>
          ))}
        </section>
      )}
    </>
  );
}

/** A group's containers, biggest first. */
function GroupInside({ map, colors, node, onShow }: Props) {
  const ids = groupsOf(map)?.find((g) => g.id === node.id)?.containers ?? [];
  const list = containersOf(map)
    .filter((c) => ids.includes(c.id))
    .sort((a, b) => b.files - a.files);
  if (!list.length) return null;
  return (
    <section className="sys-sec">
      <h4>
        <span>Inside</span>
        <span>{list.length}</span>
      </h4>
      {list.map((c) => (
        <button
          key={c.id}
          type="button"
          className="sys-li"
          onClick={() => onShow(containerId(c.id))}
        >
          <Swatch color={colors.get(c.id) ?? "#8b8d93"} />
          <span>{c.name}</span>
          <span className="sys-n">{c.files}</span>
        </button>
      ))}
    </section>
  );
}

function Inside({ map, colors, node, onShow, onOpen }: Props) {
  const ref = node.id.slice(2);
  const comps = map.components.filter((k) => k.container === ref).sort((a, b) => b.files - a.files);
  if (!comps.length) return null;
  const color = colors.get(ref) ?? "#8b8d93";
  const shade = (i: number) => tint(color, i < LIST ? 1 - i * 0.14 : 0.25);
  const rest = comps.slice(LIST).reduce((n, k) => n + k.files, 0);
  return (
    <section className="sys-sec">
      <h4>
        <span>Inside</span>
        <span>{comps.length}</span>
      </h4>
      <div className="sys-strip">
        {comps.slice(0, LIST).map((k, i) => (
          <span key={k.id} style={{ flex: k.files, background: shade(i) }} />
        ))}
        {rest > 0 && <span style={{ flex: rest, background: shade(LIST) }} />}
      </div>
      {comps.slice(0, LIST).map((k, i) => (
        <button
          key={k.id}
          type="button"
          className="sys-li"
          onClick={() => onShow(componentId(k.id))}
        >
          <span className="sys-sw" style={{ background: shade(i) }} />
          <span>{k.name}</span>
          <span className="sys-n">{k.files}</span>
        </button>
      ))}
      {comps.length > LIST && (
        <button type="button" className="sys-li more" onClick={() => onOpen(node.id)}>
          <span className="sys-sw" style={{ background: shade(LIST) }} />
          {comps.length - LIST} more
        </button>
      )}
    </section>
  );
}

function Neighbors({ map, colors, node, onShow, lens }: Props) {
  const ref = node.id.slice(2);
  const pills = (ids: string[]) => (
    <div className="sys-pills">
      {ids.map((id) => {
        const info = nodeInfo(map, colors, componentId(id));
        const other =
          info?.container !== undefined &&
          info.container !== map.components.find((k) => k.id === ref)?.container;
        return (
          <button
            key={id}
            type="button"
            className={`sys-pill${isChanged(lens?.status.get(id)) ? " chg" : ""}`}
            onClick={() => onShow(componentId(id))}
          >
            {other && info && <Swatch color={info.color} />}
            {info?.name ?? id}
          </button>
        );
      })}
    </div>
  );
  const uses = map.edges.filter((e) => e.from === ref).sort((a, b) => b.imports - a.imports);
  const usedBy = map.edges.filter((e) => e.to === ref).sort((a, b) => b.imports - a.imports);
  if (!uses.length && !usedBy.length) return null;
  return (
    <section className="sys-sec">
      {!!uses.length && (
        <>
          <h4>Uses</h4>
          {pills(uses.map((e) => e.to))}
        </>
      )}
      {!!usedBy.length && (
        <>
          <h4 style={uses.length ? { marginTop: 8 } : undefined}>Used by</h4>
          {pills(usedBy.map((e) => e.from))}
        </>
      )}
    </section>
  );
}

function ComponentBody(p: Props) {
  const k = p.map.components.find((c) => componentId(c.id) === p.node.id);
  if (!k) return null;
  if (p.lens) {
    return (
      <>
        <ChangedFiles lens={p.lens} component={k.id} onFile={p.onChangedFile} />
        <LookList items={p.lens.look.get(k.id) ?? []} onLook={p.onLook} />
        <Neighbors {...p} />
        <MostUsed map={p.map} component={k.id} onFile={p.onFile} />
      </>
    );
  }
  return (
    <>
      {!!k.entry.length && (
        <section className="sys-sec">
          <h4>Start here</h4>
          {k.entry.map((f) => (
            <button key={f} type="button" className="sys-li mono" onClick={() => p.onFile(f)}>
              <span>{f.slice(f.lastIndexOf("/") + 1)}</span>
            </button>
          ))}
        </section>
      )}
      <MostUsed map={p.map} component={k.id} onFile={p.onFile} />
      <Neighbors {...p} />
    </>
  );
}

/** The selection's description, what's inside it and what it talks to. */
export function SystemDetail(p: Props) {
  const { map, colors, node } = p;
  const info = nodeInfo(map, colors, node.id);
  const container = info?.kind === "component" ? info.container : undefined;
  const where = container !== undefined ? nodeInfo(map, colors, containerId(container)) : undefined;
  const tier =
    node.kind === "component"
      ? map.description?.components.find((c) => componentId(c.id) === node.id)?.tier
      : undefined;
  const chip = tier ?? node.tech;
  const drills =
    p.openable &&
    (node.id === SYSTEM ||
      node.kind === "group" ||
      (node.kind === "container" && (!!node.tiles?.length || !!node.small)));
  const ref = node.id.slice(2);
  const status = p.lens && node.kind === "component" ? p.lens.status.get(ref) : undefined;
  const statusChip =
    node.status === "added" || status === "added"
      ? ["grn", "New"]
      : node.status === "removed"
        ? ["red", "Removed"]
        : node.status === "changed"
          ? ["amb", "Changed"]
          : node.status === "impacted"
            ? ["", "Impacted"]
            : undefined;
  const look =
    p.lens && node.kind === "container"
      ? [...p.map.components, ...p.lens.removed]
          .filter((k) => k.container === ref)
          .flatMap((k) => p.lens?.look.get(k.id) ?? [])
      : [];
  return (
    <aside className="sys-detail">
      <header className="sys-head">
        {where && container !== undefined && (
          <button
            type="button"
            className="sys-loc"
            onClick={() => p.onShow(containerId(container))}
          >
            <Swatch color={where.color} />
            {where.name}
          </button>
        )}
        <span className="sys-title">
          {node.kind === "container" && node.color && <Swatch color={node.color} />}
          {node.id === SYSTEM && (
            <span className="sys-sw round" style={{ background: "#d8d8d8" }} />
          )}
          {node.name}
          {node.kind === "component" && chip && <span className="sys-chip">{chip}</span>}
          {statusChip && <span className={`sys-chip ${statusChip[0]}`}>{statusChip[1]}</span>}
          {p.stale && (
            <span className="sys-chip" style={{ color: "var(--amber)" }}>
              Stale
            </span>
          )}
        </span>
        {node.kind !== "component" && chip && <span className="sys-chip">{chip}</span>}
        {node.description && <p>{node.description}</p>}
        <div className="sys-acts">
          {drills && (
            <button type="button" className="sys-btn" onClick={() => p.onOpen(node.id)}>
              {node.id === SYSTEM || node.kind === "group" ? "Containers" : "Components"}
              <Chevron />
            </button>
          )}
          {node.kind === "component" && node.status !== "removed" && (
            <button type="button" className="sys-btn" onClick={() => p.onSymbols(ref)}>
              Symbols
              <Chevron />
            </button>
          )}
          {p.tour && (
            <button type="button" className="sys-btn" onClick={() => p.tour && p.onTour(p.tour)}>
              <Play size={12} />
              Tour
              <span className="sys-btn-n">{p.tour.steps.length}</span>
            </button>
          )}
          {node.kind === "component" && node.path !== undefined && (
            <button type="button" className="sys-btn" onClick={() => p.onReveal(node.path ?? "")}>
              Show in Files
            </button>
          )}
          <PipeButton onClick={p.onAsk}>Ask</PipeButton>
        </div>
      </header>
      {node.id === SYSTEM && <SystemBody {...p} />}
      {node.kind === "container" &&
        (p.lens ? (
          <>
            <ChangedInside
              map={p.map}
              lens={p.lens}
              container={ref}
              color={node.color ?? "#8b8d93"}
              onShow={p.onShow}
            />
            <LookList items={look} onLook={p.onLook} />
          </>
        ) : (
          <Inside {...p} />
        ))}
      {node.kind === "group" && <GroupInside {...p} />}
      {node.kind === "component" && <ComponentBody {...p} />}
      {node.kind !== "component" && node.id !== SYSTEM && <TalksTo {...p} />}
    </aside>
  );
}
