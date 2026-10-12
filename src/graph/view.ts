import type { GraphDto } from "../bindings";
import type { Box, Tree } from "./tree";

/** What the user has opened: expanded boxes, and the nodes whose callers are shown. */
export interface View {
  expanded: ReadonlySet<string>;
  callersOpen: ReadonlySet<number>;
}

/** Changed symbols are always in the graph. A caller is in it once the callers of something
 * it uses are opened. */
export function presentNodes(graph: GraphDto, callersOpen: ReadonlySet<number>): boolean[] {
  const present = graph.nodes.map((n) => n.hop === 0);
  const byHop = [...graph.nodes].sort((a, b) => a.hop - b.hop);
  const uses = new Map<number, number[]>();
  for (const e of graph.edges) uses.set(e.from, [...(uses.get(e.from) ?? []), e.to]);
  for (const n of byHop) {
    if (n.hop === 0) continue;
    present[n.id] = (uses.get(n.id) ?? []).some((to) => present[to] && callersOpen.has(to));
  }
  return present;
}

/** Boxes with something present in them. */
export function presentBoxes(tree: Tree, present: boolean[]): Set<string> {
  const out = new Set<string>();
  present.forEach((p, id) => {
    if (!p) return;
    for (let b: Box | undefined = tree.boxes.get(tree.boxOf[id]); b; b = up(tree, b)) {
      if (out.has(b.id)) break;
      out.add(b.id);
    }
  });
  return out;
}

const up = (tree: Tree, b: Box) => (b.parent ? tree.boxes.get(b.parent) : undefined);

/** Whether a box opens to show more boxes. */
export const canExpand = (tree: Tree, shown: ReadonlySet<string>, id: string) =>
  tree.boxes.get(id)?.children.some((c) => shown.has(c)) ?? false;

/** The boxes on screen, parents before children, and the box each node shows as: itself, or
 * the collapsed box it's inside. */
export function visibleBoxes(
  tree: Tree,
  shown: ReadonlySet<string>,
  expanded: ReadonlySet<string>,
) {
  const boxes: string[] = [];
  const walk = (id: string) => {
    if (!shown.has(id)) return;
    boxes.push(id);
    if (expanded.has(id) && canExpand(tree, shown, id)) {
      for (const c of tree.boxes.get(id)?.children ?? []) walk(c);
    }
  };
  for (const r of tree.roots) walk(r);
  return boxes;
}

/** The box a node shows as: the outermost collapsed box around it, or its own. */
export function shownAs(tree: Tree, expanded: ReadonlySet<string>, node: number): string {
  const chain: string[] = [];
  for (let b = tree.boxes.get(tree.boxOf[node]); b; b = up(tree, b)) chain.push(b.id);
  for (let i = chain.length - 1; i > 0; i--) if (!expanded.has(chain[i])) return chain[i];
  return chain[0];
}

export interface Link {
  from: string;
  to: string;
  /** Every node link under it starts at a caller. */
  caller: boolean;
}

/** One link per pair of boxes on screen with uses between what they hold. */
export function links(
  graph: GraphDto,
  tree: Tree,
  present: boolean[],
  expanded: ReadonlySet<string>,
): Link[] {
  const out = new Map<string, Link>();
  for (const e of graph.edges) {
    if (!present[e.from] || !present[e.to]) continue;
    const from = shownAs(tree, expanded, e.from);
    const to = shownAs(tree, expanded, e.to);
    // A box holding the other already shows how they relate.
    if (from === to || holds(tree, from, to) || holds(tree, to, from)) continue;
    const caller = graph.nodes[e.from].status === "caller";
    const key = `${from}>${to}`;
    const had = out.get(key);
    out.set(key, { from, to, caller: (had?.caller ?? true) && caller });
  }
  return [...out.values()];
}

/** Whether box `outer` is around box `inner`. */
function holds(tree: Tree, outer: string, inner: string): boolean {
  for (let b = tree.boxes.get(inner); b?.parent; b = tree.boxes.get(b.parent)) {
    if (b.parent === outer) return true;
  }
  return false;
}

/** Callers one hop further out than each node, by node id. */
export function nextCallers(graph: GraphDto): Map<number, number[]> {
  const out = new Map<number, number[]>();
  for (const e of graph.edges) {
    const [from, to] = [graph.nodes[e.from], graph.nodes[e.to]];
    if (from.status !== "caller" || from.hop !== to.hop + 1) continue;
    out.set(to.id, [...(out.get(to.id) ?? []), from.id]);
  }
  return out;
}

/** Past this many boxes, the first view stops opening folders. */
const FIRST_VIEW_BOXES = 20;

/** The first view: folders open a level at a time while the graph stays small, files
 * closed. */
export function firstView(graph: GraphDto, tree: Tree): View {
  const callersOpen = new Set<number>();
  const shown = presentBoxes(tree, presentNodes(graph, callersOpen));
  const folders = (ids: string[]) => ids.filter((id) => tree.boxes.get(id)?.kind === "folder");
  const expanded = new Set<string>();
  let level = folders(tree.roots);
  while (level.length > 0) {
    const next = new Set([...expanded, ...level]);
    if (visibleBoxes(tree, shown, next).length > FIRST_VIEW_BOXES) break;
    for (const id of level) expanded.add(id);
    level = folders(level.flatMap((id) => tree.boxes.get(id)?.children ?? []));
  }
  return { expanded, callersOpen };
}

/** Opens `node`'s callers, or closes them, and expands the boxes around the callers so they
 * show. */
export function toggleCallers(graph: GraphDto, tree: Tree, view: View, node: number): View {
  const callersOpen = new Set(view.callersOpen);
  if (callersOpen.has(node)) {
    callersOpen.delete(node);
    return { ...view, callersOpen };
  }
  callersOpen.add(node);
  return {
    expanded: expandAround(tree, view.expanded, nextCallers(graph).get(node) ?? []),
    callersOpen,
  };
}

/** Opens or closes the callers of every changed symbol. */
export function toggleAllCallers(graph: GraphDto, tree: Tree, view: View, open: boolean): View {
  if (!open) return { ...view, callersOpen: new Set() };
  const next = nextCallers(graph);
  const changed = graph.nodes.filter((n) => n.hop === 0 && next.has(n.id)).map((n) => n.id);
  const callers = changed.flatMap((id) => next.get(id) ?? []);
  return {
    expanded: expandAround(tree, view.expanded, callers),
    callersOpen: new Set([...view.callersOpen, ...changed]),
  };
}

function expandAround(tree: Tree, expanded: ReadonlySet<string>, nodes: number[]) {
  const out = new Set(expanded);
  for (const n of nodes) {
    const own = tree.boxes.get(tree.boxOf[n]);
    for (let b = own && up(tree, own); b; b = up(tree, b)) out.add(b.id);
  }
  return out;
}
