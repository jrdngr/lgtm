import type { ArchDto, SysComponentDto, SystemDto } from "../bindings";
import { LOOSE, PALETTE } from "./colors";

export type Level =
  | { kind: "context" }
  | { kind: "containers" }
  | { kind: "components"; container: string };

export type NodeKind = "person" | "system" | "external" | "container" | "component";

export type Status = "added" | "changed" | "removed";

export interface SNode {
  /** `p:`, `x:`, `c:` or `k:` and the person's or external's name, or the container's or
   * component's id; `sys` for the system. */
  id: string;
  kind: NodeKind;
  name: string;
  /** A container's or component's repo-relative path. */
  path?: string;
  description?: string;
  /** A container's tech, or an external's kind. */
  tech?: string;
  /** The container's color, for containers and components. */
  color?: string;
  /** A container's components' file counts, largest first. */
  tiles?: number[];
  /** The system's containers, at the context level. */
  blocks?: { name: string; color: string; files: number }[];
  /** Another container, at the edge of a components view. */
  door?: boolean;
  status?: Status;
}

export interface SEdge {
  from: string;
  to: string;
  /** Import sites, 0 for a described relation alone. */
  imports: number;
  label?: string;
  status?: "added" | "removed";
}

export interface Row {
  /** A tier's name, on the first of its rows. */
  label?: string;
  nodes: SNode[];
}

export interface Scene {
  /** A column left of the boundary, its rows, and a column right of it. */
  left: SNode[];
  inside: Row[];
  right: SNode[];
  /** Names the boundary; absent when there's none. */
  boundary?: string;
  /** The boundary's color. */
  color?: string;
  edges: SEdge[];
}

/** What the review changes, keyed by component id. */
export interface Overlay {
  status: Map<string, Status>;
  removed: SysComponentDto[];
  edges: { from: string; to: string; status: "added" | "removed" }[];
}

export const personId = (name: string) => `p:${name}`;
export const externalId = (name: string) => `x:${name}`;
export const containerId = (id: string) => `c:${id}`;
export const componentId = (id: string) => `k:${id}`;
export const SYSTEM = "sys";

/** Past this many nodes a row wraps. */
const PER_ROW = 6;
/** Containers are bigger, so their rows wrap sooner. */
const CONTAINERS_PER_ROW = 3;
/** Past this many a container card stops adding tiles. */
const TILES = 24;

export function overlayOf(arch: ArchDto): Overlay {
  const status = new Map<string, Status>();
  const removed: SysComponentDto[] = [];
  for (const c of arch.components) {
    if (c.status === "added") status.set(c.id, "added");
    if (c.status === "changed") status.set(c.id, "changed");
    if (c.status === "removed") {
      status.set(c.id, "removed");
      removed.push({
        id: c.id,
        name: c.name,
        container: c.container,
        files: c.files.length,
        entry: [],
      });
    }
  }
  const edges = arch.edges.flatMap((e) =>
    e.status === "same" ? [] : [{ from: e.from, to: e.to, status: e.status }],
  );
  return { status, removed, edges };
}

/** A container: one from the code, or one only the description has. */
export interface ContainerInfo {
  id: string;
  name: string;
  tech?: string;
  description?: string;
  files: number;
  components: number;
}

export function containersOf(map: SystemDto): ContainerInfo[] {
  const desc = map.description?.containers ?? [];
  const code = map.containers.map((c) => {
    const d = desc.find((x) => x.id === c.id);
    return {
      id: c.id,
      name: d?.name || c.name,
      tech: d?.tech,
      description: d?.description,
      files: c.files,
      components: map.components.filter((k) => k.container === c.id).length,
    };
  });
  const extra = desc
    .filter((d) => !map.containers.some((c) => c.id === d.id))
    .map((d) => ({ ...d, files: 0, components: 0 }));
  return [...code, ...extra];
}

/** Each container's color, by its id: the biggest first along the palette, files outside
 * every source root grey. */
export function containerColors(map: SystemDto): Map<string, string> {
  const sorted = containersOf(map)
    .filter((c) => c.id !== "")
    .sort((a, b) => b.files - a.files || a.id.localeCompare(b.id));
  const out = new Map(sorted.map((c, i) => [c.id, PALETTE[i % PALETTE.length]]));
  out.set("", LOOSE);
  return out;
}

function wrap(nodes: SNode[], label?: string, per = PER_ROW): Row[] {
  const rows: Row[] = [];
  for (let i = 0; i < nodes.length; i += per) {
    rows.push({ label: i === 0 ? label : undefined, nodes: nodes.slice(i, i + per) });
  }
  return rows;
}

function people(map: SystemDto): SNode[] {
  return (map.description?.people ?? []).map((p) => ({
    id: personId(p.name),
    kind: "person",
    name: p.name,
    description: p.description,
  }));
}

function externals(map: SystemDto): SNode[] {
  return (map.description?.externals ?? []).map((x) => ({
    id: externalId(x.name),
    kind: "external",
    name: x.name,
    tech: x.kind || undefined,
    description: x.description,
  }));
}

/** Adds `e` to `edges`, merging it into an edge between the same nodes. */
function add(edges: Map<string, SEdge>, e: SEdge) {
  const key = `${e.from}>${e.to}`;
  const had = edges.get(key);
  if (!had) return edges.set(key, e);
  had.imports += e.imports;
  if (e.label && !had.label?.split(" · ").includes(e.label)) {
    had.label = had.label ? `${had.label} · ${e.label}` : e.label;
  }
}

/** Relations between people, externals and containers, with containers through `container`. */
function relations(map: SystemDto, container: (id: string) => string | undefined) {
  const d = map.description;
  const node = (ref: string) => {
    if (d?.people.some((p) => p.name === ref)) return personId(ref);
    if (d?.externals.some((x) => x.name === ref)) return externalId(ref);
    return container(ref);
  };
  const out: SEdge[] = [];
  for (const r of d?.relations ?? []) {
    const from = node(r.from);
    const to = node(r.to);
    if (from && to && from !== to) out.push({ from, to, imports: 0, label: r.label });
  }
  return out;
}

function contextScene(map: SystemDto): Scene {
  const containers = containersOf(map);
  const colors = containerColors(map);
  const system: SNode = {
    id: SYSTEM,
    kind: "system",
    name: map.repo,
    description: map.description?.summary,
    blocks: containers
      .filter((c) => c.id !== "")
      .map((c) => ({ name: c.name, color: colors.get(c.id) ?? LOOSE, files: c.files }))
      .sort((a, b) => b.files - a.files),
  };
  const known = new Set(containers.map((c) => c.id));
  const edges = new Map<string, SEdge>();
  for (const e of relations(map, (id) => (known.has(id) ? SYSTEM : undefined))) add(edges, e);
  return {
    left: people(map),
    inside: [{ nodes: [system] }],
    right: externals(map),
    edges: [...edges.values()],
  };
}

function containersScene(map: SystemDto, overlay?: Overlay): Scene {
  // The biggest first, then ones only the description has, then files outside every root.
  const all = containersOf(map).sort(
    (a, b) =>
      Number(a.id === "") - Number(b.id === "") ||
      Number(!a.files) - Number(!b.files) ||
      b.files - a.files,
  );
  const colors = containerColors(map);
  const changed = (id: string) =>
    [...(overlay?.status ?? [])].some(
      ([comp, s]) =>
        s !== "removed" && map.components.some((k) => k.id === comp && k.container === id),
    ) || !!overlay?.removed.some((k) => k.container === id);
  const nodes: SNode[] = all.map((c) => ({
    id: containerId(c.id),
    kind: "container",
    name: c.name,
    path: c.id,
    tech: c.tech || undefined,
    description: c.description,
    color: colors.get(c.id),
    tiles: map.components
      .filter((k) => k.container === c.id)
      .map((k) => k.files)
      .sort((a, b) => b - a)
      .slice(0, TILES),
    status: changed(c.id) ? "changed" : undefined,
  }));
  const of = new Map(map.components.map((k) => [k.id, k.container]));
  const edges = new Map<string, SEdge>();
  for (const e of map.edges) {
    const a = of.get(e.from);
    const b = of.get(e.to);
    if (a !== undefined && b !== undefined && a !== b) {
      add(edges, { from: containerId(a), to: containerId(b), imports: e.imports });
    }
  }
  const known = new Set(all.map((c) => c.id));
  for (const e of relations(map, (id) => (known.has(id) ? containerId(id) : undefined))) {
    add(edges, e);
  }
  return {
    left: people(map),
    inside: wrap(nodes, undefined, CONTAINERS_PER_ROW),
    right: externals(map),
    boundary: map.repo,
    edges: [...edges.values()],
  };
}

/** Each component's tier by imports, 0 to 2: mostly importing puts it at the top, mostly
 * imported at the bottom. Cycles don't change it. */
export function computedTiers(
  ids: string[],
  edges: { from: string; to: string; imports: number }[],
) {
  const known = new Set(ids);
  const out = new Map<string, number>();
  const into = new Map<string, number>();
  for (const e of edges) {
    if (e.from === e.to || !known.has(e.from) || !known.has(e.to)) continue;
    out.set(e.from, (out.get(e.from) ?? 0) + e.imports);
    into.set(e.to, (into.get(e.to) ?? 0) + e.imports);
  }
  return new Map(
    ids.map((id) => {
      const o = out.get(id) ?? 0;
      const i = into.get(id) ?? 0;
      const share = o + i ? o / (o + i) : 0.5;
      return [id, share >= 2 / 3 ? 0 : share <= 1 / 3 ? 2 : 1];
    }),
  );
}

function componentsScene(map: SystemDto, container: string, overlay?: Overlay): Scene {
  const colors = containerColors(map);
  const color = colors.get(container);
  const desc = new Map((map.description?.components ?? []).map((c) => [c.id, c]));
  const comps = map.components.filter((k) => k.container === container);
  const removed = (overlay?.removed ?? []).filter(
    (k) => k.container === container && !comps.some((c) => c.id === k.id),
  );
  const node = (k: SysComponentDto, status?: Status): SNode => ({
    id: componentId(k.id),
    kind: "component",
    name: k.name,
    path: k.id,
    description: desc.get(k.id)?.description,
    color,
    status,
  });
  const nodes = comps.map((k) => node(k, overlay?.status.get(k.id)));
  const tiers = map.description?.tiers ?? [];
  const described = comps.some((k) => desc.get(k.id)?.tier);
  let inside: Row[];
  if (described) {
    const tierOf = (n: SNode) => desc.get(n.id.slice(2))?.tier ?? undefined;
    inside = [
      ...tiers.flatMap((t) =>
        wrap(
          nodes.filter((n) => tierOf(n) === t),
          t,
        ),
      ),
      ...wrap(
        nodes.filter((n) => !tierOf(n)),
        "Other",
      ),
    ];
  } else {
    const ranks = computedTiers(
      comps.map((k) => k.id),
      map.edges,
    );
    inside = [0, 1, 2].flatMap((r) => wrap(nodes.filter((n) => ranks.get(n.id.slice(2)) === r)));
  }
  inside.push(
    ...wrap(
      removed.map((k) => node(k, "removed")),
      removed.length ? "Removed" : undefined,
    ),
  );
  const ids = new Set([...comps, ...removed].map((k) => k.id));
  const of = new Map(map.components.map((k) => [k.id, k.container]));
  const edges = new Map<string, SEdge>();
  const status = new Map((overlay?.edges ?? []).map((e) => [`${e.from}>${e.to}`, e.status]));
  const doors = new Set<string>();
  for (const e of map.edges) {
    const inFrom = ids.has(e.from);
    const inTo = ids.has(e.to);
    if (inFrom && inTo) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: e.imports,
        status: status.get(`${e.from}>${e.to}`) === "added" ? "added" : undefined,
      });
    } else if (inFrom || inTo) {
      const other = of.get(inFrom ? e.to : e.from);
      if (other === undefined) continue;
      doors.add(other);
      const door = containerId(other);
      add(edges, {
        from: inFrom ? componentId(e.from) : door,
        to: inFrom ? door : componentId(e.to),
        imports: e.imports,
      });
    }
  }
  for (const e of overlay?.edges ?? []) {
    if (e.status === "removed" && ids.has(e.from) && ids.has(e.to)) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: 0,
        status: "removed",
      });
    }
  }
  const containers = containersOf(map);
  const name = containers.find((c) => c.id === container)?.name ?? container;
  return {
    left: [],
    inside: inside.filter((r) => r.nodes.length),
    right: containers
      .filter((c) => doors.has(c.id))
      .map((c) => ({
        id: containerId(c.id),
        kind: "container",
        name: c.name,
        path: c.id,
        color: colors.get(c.id),
        door: true,
      })),
    boundary: name,
    color,
    edges: [...edges.values()],
  };
}

export function sceneOf(map: SystemDto, level: Level, overlay?: Overlay): Scene {
  if (level.kind === "context") return contextScene(map);
  if (level.kind === "containers") return containersScene(map, overlay);
  return componentsScene(map, level.container, overlay);
}

/** The container a Components view opens on: the one with the most files. */
export function largestContainer(map: SystemDto): string | undefined {
  return [...map.containers].sort((a, b) => b.files - a.files)[0]?.id;
}

/** Where `id` sits: the level that shows it and the container it's in, if any. */
export function levelOf(map: SystemDto, id: string): Level | undefined {
  if (id === SYSTEM || id.startsWith("p:") || id.startsWith("x:")) return { kind: "context" };
  if (id.startsWith("c:")) return { kind: "containers" };
  const k = map.components.find((c) => componentId(c.id) === id);
  return k ? { kind: "components", container: k.container } : undefined;
}
