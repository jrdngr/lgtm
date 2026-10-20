import type { SysComponentDto, SystemDto } from "../bindings";
import { LOOSE, NEUTRAL, PALETTE } from "./colors";
import { type Group, groupOfContainer, groupsOf } from "./groups";
import { addDelta, type Delta, isChanged, type Lens } from "./lens";

export type Level =
  | { kind: "context" }
  /** With `group`, that group's containers, the other groups as doors; else every group's. */
  | { kind: "containers"; group?: string }
  | { kind: "components"; container: string }
  | { kind: "symbols"; component: string };

export type NodeKind = "person" | "system" | "external" | "container" | "component" | "group";

/** What the review does to a node: changes it, or reaches it through what it changes. */
export type Status = "added" | "changed" | "removed" | "upstream" | "impacted";

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
  /** A container's components, largest first, marked when the review changes them. */
  tiles?: { files: number; changed: boolean }[];
  /** The system's containers, at the context level. */
  blocks?: { name: string; color: string; files: number }[];
  /** Another container, at the edge of a components view, or another group at a group's. */
  door?: boolean;
  /** A container's card in its group's frame: its name, tech and change alone. */
  small?: boolean;
  /** A group drawn as a frame around its containers' cards. */
  frame?: boolean;
  status?: Status;
  /** Rows the review adds and removes in it. */
  delta?: Delta;
  /** It has open Look items. */
  flag?: boolean;
}

export interface SEdge {
  from: string;
  to: string;
  /** Import sites, 0 for a described relation alone. */
  imports: number;
  label?: string;
  status?: "added" | "removed";
  /** Between containers in different groups' frames: drawn only when lit, while the edge
   * between their groups stands for it at rest. */
  detail?: boolean;
}

export interface Row {
  /** A tier's name, on the first of its rows. */
  label?: string;
  nodes: SNode[];
}

/** A group's frame and the cards inside it. */
export interface Frame {
  node: SNode;
  nodes: SNode[];
}

export interface Scene {
  /** A column left of the boundary, its rows, and a column right of it. */
  left: SNode[];
  inside: Row[];
  /** Packs the inside nodes into this many columns instead of rows. */
  pack?: number;
  /** Group frames, packed in columns, in place of rows. */
  frames?: Frame[];
  right: SNode[];
  /** Names the boundary; absent when there's none. */
  boundary?: string;
  /** The boundary's color. */
  color?: string;
  edges: SEdge[];
}

/** Every node inside a scene's boundary: its rows', or its frames and their cards. */
export const nodesOf = (s: Scene) => [
  ...s.inside.flatMap((r) => r.nodes),
  ...(s.frames ?? []).flatMap((f) => [f.node, ...f.nodes]),
];

export const personId = (name: string) => `p:${name}`;
export const externalId = (name: string) => `x:${name}`;
export const containerId = (id: string) => `c:${id}`;
export const componentId = (id: string) => `k:${id}`;
export { groupId } from "./groups";
export const SYSTEM = "sys";

/** Past this many nodes a row wraps. */
const PER_ROW = 6;
/** Containers are bigger, so their rows wrap sooner. */
const CONTAINERS_PER_ROW = 3;
/** Past this many a container card stops adding tiles. */
const TILES = 24;

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

/** Blocks for containers, biggest first. */
function blocksOf(map: SystemDto, ids: string[]) {
  const colors = containerColors(map);
  return containersOf(map)
    .filter((c) => c.id !== "" && ids.includes(c.id))
    .map((c) => ({ name: c.name, color: colors.get(c.id) ?? LOOSE, files: c.files }))
    .sort((a, b) => b.files - a.files);
}

/** Import edges between the nodes `of` puts containers' components in. */
function importEdges(map: SystemDto, of: (container: string) => string | undefined) {
  const comp = new Map(map.components.map((k) => [k.id, k.container]));
  const out: SEdge[] = [];
  for (const e of map.edges) {
    const a = comp.get(e.from);
    const b = comp.get(e.to);
    const from = a === undefined ? undefined : of(a);
    const to = b === undefined ? undefined : of(b);
    if (from && to && from !== to) out.push({ from, to, imports: e.imports });
  }
  return out;
}

function groupNode(map: SystemDto, g: Group): SNode {
  return {
    id: g.id,
    kind: "group",
    name: g.name,
    description: g.description,
    color: NEUTRAL,
    blocks: blocksOf(map, g.containers),
  };
}

/** Groups in the system, wired to each other by imports and to people and externals. */
function groupedContext(map: SystemDto, groups: Group[]): Scene {
  const of = groupOfContainer(groups);
  const edges = new Map<string, SEdge>();
  for (const e of importEdges(map, (c) => of.get(c))) add(edges, e);
  for (const e of relations(map, (id) => of.get(id))) add(edges, e);
  return {
    left: people(map),
    inside: [{ nodes: groups.map((g) => groupNode(map, g)) }],
    pack: 2,
    right: externals(map),
    boundary: map.repo,
    edges: [...edges.values()],
  };
}

function contextScene(map: SystemDto): Scene {
  const groups = groupsOf(map);
  if (groups) return groupedContext(map, groups);
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

function containersScene(map: SystemDto, lens?: Lens, group?: string): Scene {
  // The biggest first, then ones only the description has, then files outside every root.
  const all = containersOf(map).sort(
    (a, b) =>
      Number(a.id === "") - Number(b.id === "") ||
      Number(!a.files) - Number(!b.files) ||
      b.files - a.files,
  );
  const colors = containerColors(map);
  const nodes: SNode[] = all.map((c) => {
    const comps = [
      ...map.components.filter((k) => k.container === c.id),
      ...(lens?.removed ?? []).filter((k) => k.container === c.id),
    ];
    const changed = comps.filter((k) => isChanged(lens?.status.get(k.id)));
    const impacted = comps.some((k) => lens?.status.get(k.id) === "impacted");
    return {
      id: containerId(c.id),
      kind: "container",
      name: c.name,
      path: c.id,
      tech: c.tech || undefined,
      description: c.description,
      color: colors.get(c.id),
      tiles: comps
        .filter((k) => lens?.status.get(k.id) !== "removed")
        .sort((a, b) => b.files - a.files)
        .slice(0, TILES)
        .map((k) => ({ files: k.files, changed: changed.includes(k) })),
      status: changed.length ? "changed" : impacted ? "impacted" : undefined,
      delta: changed.reduce<Delta | undefined>(
        (d, k) => addDelta(d, lens?.delta.get(k.id)),
        undefined,
      ),
      flag: comps.some((k) => lens?.look.has(k.id)),
    };
  });
  const of = new Map(map.components.map((k) => [k.id, k.container]));
  const edges = new Map<string, SEdge>();
  for (const e of map.edges) {
    const a = of.get(e.from);
    const b = of.get(e.to);
    if (a !== undefined && b !== undefined && a !== b) {
      add(edges, { from: containerId(a), to: containerId(b), imports: e.imports });
    }
  }
  if (lens) lensEdges(map, lens, edges);
  const known = new Set(all.map((c) => c.id));
  for (const e of relations(map, (id) => (known.has(id) ? containerId(id) : undefined))) {
    add(edges, e);
  }
  const scene: Scene = {
    left: people(map),
    inside: wrap(nodes, undefined, CONTAINERS_PER_ROW),
    right: externals(map),
    boundary: map.repo,
    edges: [...edges.values()],
  };
  const groups = groupsOf(map);
  if (!groups) return scene;
  const open = groups.find((g) => g.id === group);
  return open ? groupScene(scene, groups, open) : framedScene(scene, groups);
}

/** Every group as a frame of small container cards. At rest the edges between groups stand
 * for the ones between their containers. */
function framedScene(scene: Scene, groups: Group[]): Scene {
  const nodes = new Map(scene.inside.flatMap((r) => r.nodes).map((n) => [n.id, n]));
  const of = groupOfContainer(groups);
  const groupOf = (id: string) => (id.startsWith("c:") ? of.get(id.slice(2)) : id);
  const frames = groups.map((g) => {
    const cards = g.containers.flatMap((c) => {
      const n = nodes.get(containerId(c));
      return n ? [{ ...n, small: true, tiles: undefined }] : [];
    });
    const changed = cards.filter((n) => n.status === "changed");
    const node: SNode = {
      id: g.id,
      kind: "group",
      name: g.name,
      description: g.description,
      color: NEUTRAL,
      frame: true,
      status: changed.length ? "changed" : cards.some((n) => n.status) ? "impacted" : undefined,
      delta: changed.reduce<Delta | undefined>((d, n) => addDelta(d, n.delta), undefined),
      flag: cards.some((n) => n.flag),
    };
    return { node, nodes: cards };
  });
  const bundles = new Map<string, SEdge>();
  const detail: SEdge[] = [];
  for (const e of scene.edges) {
    const from = groupOf(e.from);
    const to = groupOf(e.to);
    if (!from || !to) continue;
    if (from !== to) add(bundles, { from, to, imports: e.imports, label: e.label });
    if (from !== e.from || to !== e.to) detail.push({ ...e, detail: true });
  }
  for (const b of bundles.values()) {
    const statuses = scene.edges
      .filter((e) => groupOf(e.from) === b.from && groupOf(e.to) === b.to)
      .map((e) => e.status);
    if (statuses.every((x) => x === "added")) b.status = "added";
    else if (statuses.every((x) => x === "removed")) b.status = "removed";
  }
  return { ...scene, inside: [], frames, edges: [...bundles.values(), ...detail] };
}

/** One group's containers, with the other groups it talks to as doors. */
function groupScene(scene: Scene, groups: Group[], open: Group): Scene {
  const of = groupOfContainer(groups);
  const mine = new Set(open.containers.map(containerId));
  const end = (id: string) => (mine.has(id) || !id.startsWith("c:") ? id : of.get(id.slice(2)));
  const edges = new Map<string, SEdge>();
  for (const e of scene.edges) {
    if (!mine.has(e.from) && !mine.has(e.to)) continue;
    const from = end(e.from);
    const to = end(e.to);
    if (from && to && from !== to) add(edges, { ...e, from, to });
  }
  const touched = new Set([...edges.values()].flatMap((e) => [e.from, e.to]));
  const doors: SNode[] = groups
    .filter((g) => g !== open && touched.has(g.id))
    .map((g) => ({ id: g.id, kind: "group", name: g.name, color: NEUTRAL, door: true }));
  const nodes = scene.inside.flatMap((r) => r.nodes).filter((n) => mine.has(n.id));
  return {
    left: scene.left.filter((n) => touched.has(n.id)),
    inside: wrap(nodes, undefined, CONTAINERS_PER_ROW),
    right: [...doors, ...scene.right.filter((n) => touched.has(n.id))],
    boundary: open.name,
    edges: [...edges.values()],
  };
}

/** Marks container edges that only the review's new imports make as added, and adds those
 * only its removed imports made. */
function lensEdges(map: SystemDto, lens: Lens, edges: Map<string, SEdge>) {
  const of = new Map(
    [...map.components, ...lens.removed].map((k) => [k.id, containerId(k.container)]),
  );
  const added = new Set(lens.edges.filter((e) => e.status === "added").map(edgeOf));
  const kept = new Set<string>();
  for (const e of map.edges) {
    const a = of.get(e.from);
    const b = of.get(e.to);
    if (a && b && a !== b && !added.has(edgeOf(e))) kept.add(`${a}>${b}`);
  }
  for (const e of lens.edges) {
    const a = of.get(e.from);
    const b = of.get(e.to);
    if (!a || !b || a === b) continue;
    const key = `${a}>${b}`;
    const had = edges.get(key);
    if (e.status === "added" && had && !kept.has(key)) had.status = "added";
    if (e.status === "removed" && !had) {
      edges.set(key, { from: a, to: b, imports: e.imports, status: "removed" });
    }
  }
}

const edgeOf = (e: { from: string; to: string }) => `${e.from}>${e.to}`;

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

function componentsScene(map: SystemDto, container: string, lens?: Lens): Scene {
  const colors = containerColors(map);
  const color = colors.get(container);
  const desc = new Map((map.description?.components ?? []).map((c) => [c.id, c]));
  const comps = map.components.filter((k) => k.container === container);
  const removed = (lens?.removed ?? []).filter(
    (k) => k.container === container && !comps.some((c) => c.id === k.id),
  );
  const node = (k: SysComponentDto): SNode => {
    const s = lens?.status.get(k.id);
    return {
      id: componentId(k.id),
      kind: "component",
      name: k.name,
      path: k.id,
      description: desc.get(k.id)?.description,
      color,
      status: s === "unchanged" ? undefined : s,
      delta: lens?.delta.get(k.id),
      flag: lens?.look.has(k.id),
    };
  };
  const nodes = comps.map(node);
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
  inside.push(...wrap(removed.map(node), removed.length ? "Removed" : undefined));
  const ids = new Set([...comps, ...removed].map((k) => k.id));
  const of = new Map(map.components.map((k) => [k.id, k.container]));
  const edges = new Map<string, SEdge>();
  const status = new Map((lens?.edges ?? []).map((e) => [edgeOf(e), e.status]));
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
  for (const e of lens?.edges ?? []) {
    if (e.status === "removed" && ids.has(e.from) && ids.has(e.to)) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: e.imports,
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

/** With the lens: only what the review changes or impacts, and what wires to it, unless `all`
 * keeps the rest too. */
export function sceneOf(map: SystemDto, level: Level, lens?: Lens, all = false): Scene {
  if (level.kind === "context") return contextScene(map);
  const only = lens && !all;
  if (level.kind === "containers") {
    const scene = containersScene(map, lens, level.group);
    return only ? affectedOnly(scene, CONTAINERS_PER_ROW) : scene;
  }
  const container = level.kind === "symbols" ? containerOf(map, level.component) : level.container;
  const scene = componentsScene(map, container, lens);
  return only ? affectedOnly(scene, PER_ROW) : scene;
}

/** The ids of the nodes the lens keeps in `scene`. */
export function affectedIds(scene: Scene): Set<string> {
  const kept = affectedOnly(scene, PER_ROW);
  return new Set([...nodesOf(kept), ...kept.left, ...kept.right].map((n) => n.id));
}

/** Whether the review changes a node or reaches it through a change it uses. */
const affected = (n: SNode) => !!n.status && n.status !== "upstream";

/** `scene` without the nodes the review doesn't affect, rows packed again within each tier.
 * The whole scene when it affects none. */
export function affectedOnly(scene: Scene, per: number): Scene {
  if (scene.frames) return affectedFrames(scene);
  const groups: Row[] = [];
  for (const r of scene.inside) {
    const last = groups.at(-1);
    if (!last || r.label !== undefined || last.label === undefined) {
      groups.push({ label: r.label, nodes: [...r.nodes] });
    } else last.nodes.push(...r.nodes);
  }
  const inside = groups.flatMap((g) => wrap(g.nodes.filter(affected), g.label, per));
  if (!inside.length) return scene;
  const ids = new Set(inside.flatMap((r) => r.nodes.map((n) => n.id)));
  return { ...keepWired(scene, ids), inside };
}

function affectedFrames(scene: Scene): Scene {
  const frames = (scene.frames ?? [])
    .map((f) => ({ ...f, nodes: f.nodes.filter(affected) }))
    .filter((f) => f.nodes.length);
  if (!frames.length) return scene;
  const ids = new Set(frames.flatMap((f) => [f.node.id, ...f.nodes.map((n) => n.id)]));
  return { ...keepWired(scene, ids), frames };
}

/** `scene`'s side nodes and edges that still join the `ids` it keeps inside. */
function keepWired(scene: Scene, ids: Set<string>): Scene {
  const touching = scene.edges.filter((e) => ids.has(e.from) || ids.has(e.to));
  const wired = (n: SNode) => touching.some((e) => e.from === n.id || e.to === n.id);
  const left = scene.left.filter(wired);
  const right = scene.right.filter(wired);
  for (const n of [...left, ...right]) ids.add(n.id);
  return {
    ...scene,
    left,
    right,
    edges: touching.filter((e) => ids.has(e.from) && ids.has(e.to)),
  };
}

/** The container a component is in. */
export const containerOf = (map: SystemDto, component: string) =>
  map.components.find((k) => k.id === component)?.container ?? "";

/** The container a Components view opens on: the one with the most files. */
export function largestContainer(map: SystemDto): string | undefined {
  return [...map.containers].sort((a, b) => b.files - a.files)[0]?.id;
}

/** Where `id` sits: the level that shows it and the container it's in, if any. */
export function levelOf(map: SystemDto, id: string): Level | undefined {
  if (id === SYSTEM || id.startsWith("p:") || id.startsWith("x:") || id.startsWith("g:")) {
    return { kind: "context" };
  }
  if (id.startsWith("c:")) return { kind: "containers" };
  const k = map.components.find((c) => componentId(c.id) === id);
  return k ? { kind: "components", container: k.container } : undefined;
}
