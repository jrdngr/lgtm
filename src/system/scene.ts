import type { ArchDto, SysComponentDto, SystemDto } from "../bindings";

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
  /** The kind line above the name. */
  label: string;
  /** A container's or component's repo-relative path. */
  path?: string;
  description?: string;
  meta?: string;
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
  /** Rows above the boundary, inside it, and below it. */
  above: Row[];
  inside: Row[];
  below: Row[];
  /** Names the boundary; absent when there's none. */
  boundary?: string;
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

export function overlayOf(arch: ArchDto): Overlay {
  const status = new Map<string, Status>();
  const removed: SysComponentDto[] = [];
  for (const c of arch.components) {
    if (c.status === "added") status.set(c.id, "added");
    if (c.status === "changed") status.set(c.id, "changed");
    if (c.status === "removed") {
      status.set(c.id, "removed");
      removed.push({ id: c.id, name: c.name, container: c.container, files: c.files.length });
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

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

function wrap(nodes: SNode[], label?: string): Row[] {
  const rows: Row[] = [];
  for (let i = 0; i < nodes.length; i += PER_ROW) {
    rows.push({ label: i === 0 ? label : undefined, nodes: nodes.slice(i, i + PER_ROW) });
  }
  return rows;
}

function people(map: SystemDto): SNode[] {
  return (map.description?.people ?? []).map((p) => ({
    id: personId(p.name),
    kind: "person",
    name: p.name,
    label: "Person",
    description: p.description,
  }));
}

function externals(map: SystemDto): SNode[] {
  return (map.description?.externals ?? []).map((x) => ({
    id: externalId(x.name),
    kind: "external",
    name: x.name,
    label: x.kind ? `External · ${x.kind}` : "External",
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
  const system: SNode = {
    id: SYSTEM,
    kind: "system",
    name: map.repo,
    label: "Software system",
    description: map.description?.summary,
    meta: `${plural(containers.length, "container")} · ${plural(map.components.length, "component")}`,
  };
  const known = new Set(containers.map((c) => c.id));
  const edges = new Map<string, SEdge>();
  for (const e of relations(map, (id) => (known.has(id) ? SYSTEM : undefined))) add(edges, e);
  return {
    above: wrap(people(map)),
    inside: [{ nodes: [system] }],
    below: wrap(externals(map)),
    edges: [...edges.values()],
  };
}

function containersScene(map: SystemDto, overlay?: Overlay): Scene {
  const all = containersOf(map);
  const changed = (id: string) =>
    [...(overlay?.status ?? [])].some(
      ([comp, s]) =>
        s !== "removed" && map.components.some((k) => k.id === comp && k.container === id),
    ) || !!overlay?.removed.some((k) => k.container === id);
  const nodes: SNode[] = all.map((c) => ({
    id: containerId(c.id),
    kind: "container",
    name: c.name,
    label: [c.tech ? `Container · ${c.tech}` : "Container", c.id].filter(Boolean).join(" · "),
    description: c.description,
    meta: c.components ? plural(c.components, "component") : undefined,
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
    above: wrap(people(map)),
    inside: wrap(nodes),
    below: wrap(externals(map)),
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
  const desc = new Map((map.description?.components ?? []).map((c) => [c.id, c]));
  const comps = map.components.filter((k) => k.container === container);
  const removed = (overlay?.removed ?? []).filter(
    (k) => k.container === container && !comps.some((c) => c.id === k.id),
  );
  const node = (k: SysComponentDto, status?: Status): SNode => ({
    id: componentId(k.id),
    kind: "component",
    name: k.name,
    label: "Component",
    path: k.id,
    description: desc.get(k.id)?.description,
    meta: plural(k.files, "file"),
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
  const edges = new Map<string, SEdge>();
  const status = new Map((overlay?.edges ?? []).map((e) => [`${e.from}>${e.to}`, e.status]));
  for (const e of map.edges) {
    if (ids.has(e.from) && ids.has(e.to)) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: e.imports,
        status: status.get(`${e.from}>${e.to}`) === "added" ? "added" : undefined,
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
  const name = containersOf(map).find((c) => c.id === container)?.name ?? container;
  return {
    above: [],
    inside: inside.filter((r) => r.nodes.length),
    below: [],
    boundary: name,
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
