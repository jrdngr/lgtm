import type { SystemDto } from "../bindings";
import { componentId, containerId, externalId, type Level, personId, SYSTEM } from "./scene";

/** One step of a path: an import between components, with its first site, or a described
 * relation. */
export interface Hop {
  from: string;
  to: string;
  label?: string;
  path?: string;
  line?: number;
}

interface Link {
  /** The graph's nodes it joins. A container is two nodes: itself, which leads into its
   * components, and `^` and its id, which its components lead up to and which leads only
   * out, so a path can't pass between two components just for sharing a container. */
  a: string;
  b: string;
  /** Absent between a container and its components, which a path passes through freely. */
  hop?: Hop;
}

const up = (container: string) => `^${container}`;

function links(map: SystemDto): Link[] {
  const d = map.description;
  const ref = (r: string) => {
    if (d?.people.some((p) => p.name === r)) return personId(r);
    if (d?.externals.some((x) => x.name === r)) return externalId(r);
    return containerId(r);
  };
  const out: Link[] = map.edges.map((e) => {
    const hop = { from: componentId(e.from), to: componentId(e.to), path: e.path, line: e.line };
    return { a: hop.from, b: hop.to, hop };
  });
  for (const r of d?.relations ?? []) {
    const hop = { from: ref(r.from), to: ref(r.to), label: r.label };
    out.push({ a: hop.from, b: hop.to, hop });
    if (hop.from.startsWith("c:")) out.push({ a: up(hop.from), b: hop.to, hop });
  }
  for (const k of map.components) {
    out.push({ a: containerId(k.container), b: componentId(k.id) });
    out.push({ a: componentId(k.id), b: up(containerId(k.container)) });
  }
  return out;
}

/** The fewest hops from `from` to `to` (0-1 BFS), following links forward, or either way when
 * `undirected`. */
function search(all: Link[], from: string, to: string, undirected: boolean): Hop[] | undefined {
  const out = new Map<string, { next: string; link: Link }[]>();
  const push = (a: string, next: string, link: Link) => {
    const list = out.get(a) ?? [];
    list.push({ next, link });
    out.set(a, list);
  };
  for (const l of all) {
    push(l.a, l.b, l);
    // Backward, a container's components still lead only up to it.
    if (undirected && l.hop) push(l.b.startsWith("c:") ? up(l.b) : l.b, l.a, l);
  }
  const dist = new Map([[from, 0]]);
  const prev = new Map<string, { node: string; link: Link }>();
  const queue = [from];
  while (queue.length) {
    const node = queue.shift() as string;
    const d = dist.get(node) ?? 0;
    for (const { next, link } of out.get(node) ?? []) {
      const nd = d + (link.hop ? 1 : 0);
      if (nd >= (dist.get(next) ?? Number.POSITIVE_INFINITY)) continue;
      dist.set(next, nd);
      prev.set(next, { node, link });
      if (link.hop) queue.push(next);
      else queue.unshift(next);
    }
  }
  const ends = [to, up(to)].filter((n) => dist.has(n));
  const end = ends.sort((x, y) => (dist.get(x) ?? 0) - (dist.get(y) ?? 0))[0];
  if (end === undefined) return undefined;
  const hops: Hop[] = [];
  for (let at = end; at !== from; ) {
    const p = prev.get(at);
    if (!p) return undefined;
    if (p.link.hop) hops.unshift(p.link.hop);
    at = p.node;
  }
  return hops;
}

/** The shortest path of imports and described relations from `from` to `to`, in either
 * direction when no path follows them forward. */
export function tracePath(map: SystemDto, from: string, to: string): Hop[] | undefined {
  if (from === to) return undefined;
  const all = links(map);
  return search(all, from, to, false) ?? search(all, from, to, true);
}

/** The node that shows `id` at `level`: itself, or what holds it. */
export function shownAs(map: SystemDto, level: Level, id: string): string | undefined {
  const container = (k: string) => map.components.find((c) => componentId(c.id) === k)?.container;
  if (level.kind === "context") {
    return id.startsWith("c:") || id.startsWith("k:") ? SYSTEM : id;
  }
  if (level.kind === "containers") {
    if (!id.startsWith("k:")) return id;
    const c = container(id);
    return c === undefined ? undefined : containerId(c);
  }
  if (id.startsWith("p:") || id.startsWith("x:")) return undefined;
  const own = level.kind === "symbols" ? container(componentId(level.component)) : level.container;
  if (id === containerId(own ?? "")) return undefined;
  if (id.startsWith("c:")) return id;
  const c = container(id);
  if (c === undefined) return undefined;
  return c === own ? id : containerId(c);
}

/** A path as `level` draws it: its nodes, and its edges numbered in order. */
export function drawn(map: SystemDto, level: Level, hops: Hop[]) {
  const nodes = new Set<string>();
  const edges = new Map<string, number>();
  for (const h of hops) {
    const a = shownAs(map, level, h.from);
    const b = shownAs(map, level, h.to);
    if (a) nodes.add(a);
    if (b) nodes.add(b);
    const key = `${a}>${b}`;
    if (a && b && a !== b && !edges.has(key)) edges.set(key, edges.size + 1);
  }
  return { nodes, edges };
}
