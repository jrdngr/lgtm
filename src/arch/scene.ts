import type { ArchDto, ComponentDto, EdgeDto, IntentDto } from "../bindings";
import type { ArchSelection } from "./state";

/** Changed shows changed, upstream and impacted components; All shows every one. */
export type Mode = "changed" | "all";

export type EdgeStyle = "ctx" | "add" | "rem" | "wrong" | "imp" | "move";

/** A line in the graph: an import edge, or a Move intent's arrow to where its item belongs. */
export interface SceneEdge {
  /** `from>to` for an edge, `move:<intent>` for a Move arrow. */
  key: string;
  from: string;
  to: string;
  style: EdgeStyle;
  edge?: EdgeDto;
  intent?: number;
}

export interface SceneGroup {
  id: string;
  label: string;
  children: string[];
}

export interface Scene {
  groups: SceneGroup[];
  components: ComponentDto[];
  edges: SceneEdge[];
  /** Assess's trust boundaries, and components tagged Trust Boundary, by the components in
   * them. */
  boundaries: { name: string; components: string[] }[];
}

export const edgeKey = (from: string, to: string) => `${from}>${to}`;

/** How an edge draws: a Wrong mark it breaks first, then whether it's new, gone or carries
 * impact. */
export function edgeStyle(e: EdgeDto): EdgeStyle {
  if (e.wrong !== null && e.status !== "removed") return "wrong";
  if (e.status === "removed") return "rem";
  if (e.status === "added") return "add";
  return e.impacted ? "imp" : "ctx";
}

/** The component holding a Move intent's file or symbol. */
export function moveSource(
  arch: ArchDto,
  it: Extract<IntentDto, { kind: "move" }>,
): string | undefined {
  const item = it.symbol ? `${it.path}#${it.symbol}` : it.path;
  return (
    arch.components.find((c) => c.misplaced.includes(item))?.id ??
    arch.components.find((c) => c.files.some((f) => f.path === it.path))?.id
  );
}

/** Arrows from where each unmet Move intent's item is to where it belongs. */
export function moveEdges(arch: ArchDto): SceneEdge[] {
  const out: SceneEdge[] = [];
  for (const it of arch.intents) {
    if (it.kind !== "move" || it.state === "met") continue;
    const from = moveSource(arch, it);
    if (!from || from === it.to) continue;
    out.push({ key: `move:${it.id}`, from, to: it.to, style: "move", intent: it.id });
  }
  return out;
}

/** The components on screen, by id, in `mode`. Changed adds the ends of Move arrows and of
 * edges marked Wrong, so marks always show. */
export function shownComponents(arch: ArchDto, mode: Mode, moves: SceneEdge[]): Set<string> {
  if (mode === "all") return new Set(arch.components.map((c) => c.id));
  const out = new Set(arch.components.filter((c) => c.status !== "unchanged").map((c) => c.id));
  for (const e of moves) out.add(e.from).add(e.to);
  for (const e of arch.edges) if (e.wrong !== null) out.add(e.from).add(e.to);
  return out;
}

/** What the components level draws in `mode`: components in their containers, the edges
 * between those shown, Move arrows and trust boundaries. */
export function buildScene(arch: ArchDto, mode: Mode): Scene {
  const moves = moveEdges(arch);
  const shown = shownComponents(arch, mode, moves);
  const components = arch.components.filter((c) => shown.has(c.id));
  const edges: SceneEdge[] = arch.edges
    .filter((e) => shown.has(e.from) && shown.has(e.to) && e.from !== e.to)
    .map((e) => ({
      key: edgeKey(e.from, e.to),
      from: e.from,
      to: e.to,
      style: edgeStyle(e),
      edge: e,
    }));
  edges.push(...moves.filter((e) => shown.has(e.from) && shown.has(e.to)));
  const groups = arch.containers
    .map((c) => ({
      id: containerNode(c.id),
      label: c.name,
      children: components.filter((x) => x.container === c.id).map((x) => x.id),
    }))
    .filter((g) => g.children.length > 0);
  const boundaries = (arch.assessment?.boundaries ?? []).map((b) => ({
    name: b.name,
    components: b.components.filter((id) => shown.has(id)),
  }));
  const bounded = new Set(boundaries.flatMap((b) => b.components));
  for (const c of components) {
    if (c.trust && !bounded.has(c.id)) boundaries.push({ name: "", components: [c.id] });
  }
  return { groups, components, edges, boundaries: boundaries.filter((b) => b.components.length) };
}

/** A container's node id at the top level. Containers' own ids can be empty. */
export const containerNode = (id: string) => `c:${id}`;

/** Container links: one per pair of containers with edges between their components. Gone
 * when every edge under it is, else the first of Wrong, new, Move and impacted it has. */
export function containerEdges(arch: ArchDto, edges: SceneEdge[]): SceneEdge[] {
  const of = new Map(arch.components.map((c) => [c.id, containerNode(c.container)]));
  const under = new Map<string, SceneEdge[]>();
  for (const e of edges) {
    const [a, b] = [of.get(e.from), of.get(e.to)];
    if (a === undefined || b === undefined || a === b) continue;
    const key = edgeKey(a, b);
    under.set(key, [...(under.get(key) ?? []), e]);
  }
  return [...under.entries()].map(([key, list]) => {
    const has = (s: EdgeStyle) => list.some((e) => e.style === s);
    const style: EdgeStyle = list.every((e) => e.style === "rem")
      ? "rem"
      : ((["wrong", "add", "move", "imp"] as const).find(has) ?? "ctx");
    return { key, style, ...ends(of, list[0]) };
  });
}

const ends = (of: Map<string, string>, e: SceneEdge) => ({
  from: of.get(e.from) as string,
  to: of.get(e.to) as string,
});

/** What a selection lights up: its components and edges. Undefined when nothing is focused. */
export interface Focus {
  nodes: Set<string>;
  edges: Set<string>;
  /** The selected component, outlined apart from the others in focus. */
  primary?: string;
}

/** The components and edges `sel` is about. A component brings its edges and the components
 * at their other ends. */
export function focusOf(arch: ArchDto, edges: SceneEdge[], sel?: ArchSelection): Focus | undefined {
  if (!sel) return undefined;
  const nodes = new Set<string>();
  const keys = new Set<string>();
  const edge = (from: string, to: string, key = edgeKey(from, to)) => {
    nodes.add(from).add(to);
    keys.add(key);
  };
  switch (sel.kind) {
    case "component":
      nodes.add(sel.id);
      for (const e of edges) if (e.from === sel.id || e.to === sel.id) edge(e.from, e.to, e.key);
      break;
    case "edge":
      edge(sel.from, sel.to);
      break;
    case "layer": {
      const layer = arch.layers.find((l) => l.n === sel.n);
      for (const id of layer?.components ?? []) nodes.add(id);
      for (const e of edges) if (nodes.has(e.from) && nodes.has(e.to)) keys.add(e.key);
      break;
    }
    case "look": {
      const look = arch.look.find((l) => l.key === sel.key);
      if (look?.component) nodes.add(look.component);
      if (look?.edge) edge(look.edge[0], look.edge[1]);
      break;
    }
    case "intent": {
      const it = arch.intents.find((i) => i.id === sel.id);
      if (!it) break;
      if (it.kind === "wrong") edge(it.from, it.to);
      else if (it.kind === "move") {
        const from = moveSource(arch, it);
        if (from) edge(from, it.to, `move:${it.id}`);
        else nodes.add(it.to);
      } else nodes.add(it.component);
      break;
    }
  }
  if (nodes.size === 0 && keys.size === 0) return undefined;
  return { nodes, edges: keys, primary: sel.kind === "component" ? sel.id : undefined };
}
