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

interface Step {
  next: string;
  link: Link;
}

/** Each node's links out, following links forward, or either way when `undirected`. */
function adjacency(all: Link[], undirected: boolean) {
  const out = new Map<string, Step[]>();
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
  return out;
}

/** The fewest hops (0-1 BFS) from any of `from` to each node they reach, and the step in. */
function reach(out: Map<string, Step[]>, from: string[]) {
  const dist = new Map(from.map((n) => [n, 0]));
  const prev = new Map<string, { node: string; link: Link }>();
  const queue = [...from];
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
  return { dist, prev };
}

/** Paths at most this many hops longer than the shortest count as alternatives. */
const SLACK = 2;
/** The most links to walk looking for paths. */
const STEPS = 20_000;
export const MAX_PATHS = 5;

/** Up to `cap` simple paths from `from` to `to` that use no hop twice, fewest hops first: the
 * shortest, then those up to SLACK hops longer. Searches one length at a time, so a longer path
 * never crowds out a shorter one. */
function paths(out: Map<string, Step[]>, from: string, to: string, cap: number) {
  const back = new Map<string, Step[]>();
  for (const [a, steps] of out) {
    for (const { next, link } of steps) {
      const list = back.get(next) ?? [];
      list.push({ next: a, link });
      back.set(next, list);
    }
  }
  /** The fewest hops left from each node to `to`, reusing hops or not. */
  const left = reach(back, [to, up(to)]).dist;
  const best = left.get(from);
  if (best === undefined) return undefined;
  const found = new Map<string, Hop[]>();
  const seen = new Set([from]);
  const used = new Set<Hop>();
  const hops: Hop[] = [];
  let steps = STEPS;
  let length = best;
  const walk = (node: string) => {
    if (node === to || node === up(to)) {
      const key = hops.map((h) => `${h.from}>${h.to}`).join(" ");
      if (hops.length === length && !found.has(key)) found.set(key, [...hops]);
      return;
    }
    for (const { next, link } of out.get(node) ?? []) {
      if (found.size >= cap || --steps < 0) return;
      const n = hops.length + (link.hop ? 1 : 0);
      const rest = left.get(next) ?? Number.POSITIVE_INFINITY;
      if (seen.has(next) || (link.hop && used.has(link.hop)) || n + rest > length) continue;
      seen.add(next);
      if (link.hop) {
        hops.push(link.hop);
        used.add(link.hop);
      }
      walk(next);
      if (link.hop) {
        hops.pop();
        used.delete(link.hop);
      }
      seen.delete(next);
    }
  };
  // No path is longer than the hops there are.
  let longest = new Set([...out.values()].flatMap((s) => s.flatMap((x) => x.link.hop ?? []))).size;
  for (; length <= longest && found.size < cap && steps > 0; length++) {
    walk(from);
    const first = found.values().next().value;
    if (first) longest = Math.min(longest, first.length + SLACK);
  }
  return found.size ? [...found.values()] : undefined;
}

/** The shortest path of imports and described relations from `from` to `to`, in either
 * direction when no path follows them forward. Empty when one holds the other. */
export function tracePath(map: SystemDto, from: string, to: string): Hop[] | undefined {
  return tracePaths(map, from, to, 1)[0];
}

/** Up to `cap` paths from `from` to `to`, the shortest and those a few hops longer, fewest hops
 * first, in the direction tracePath takes. Empty when nothing joins them. */
export function tracePaths(map: SystemDto, from: string, to: string, cap = MAX_PATHS): Hop[][] {
  if (from === to) return [];
  const all = links(map);
  return (
    paths(adjacency(all, false), from, to, cap) ?? paths(adjacency(all, true), from, to, cap) ?? []
  );
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
