import type { ArchDto, GraphDto, GraphEdge, GraphNode } from "../bindings";
import { buildTree, type Tree } from "../graph/tree";
import { moveSource } from "./scene";

/** One component's symbols, with each other component they link to as one stub node. */
export interface Scoped {
  graph: GraphDto;
  /** Stub node ids to their component ids. */
  stubs: Map<number, string>;
}

/** The component each file of the architecture is in, by path. */
const componentsByPath = (arch: ArchDto) =>
  new Map(arch.components.flatMap((c) => c.files.map((f) => [f.path, c.id] as const)));

/** The symbol graph cut down to `component`'s files. A use between one of its symbols and
 * another component's becomes a use of that component's stub, and the targets of its Move and
 * Wrong marks get stubs too. */
export function scopeGraph(graph: GraphDto, arch: ArchDto, component: string): Scoped {
  const compOf = componentsByPath(arch);
  const ids = new Map<number, number>();
  const nodes: GraphNode[] = [];
  for (const n of graph.nodes) {
    if (compOf.get(n.path) !== component) continue;
    ids.set(n.id, nodes.length);
    nodes.push({ ...n, id: nodes.length });
  }
  const stubs = new Map<number, string>();
  const stubOf = new Map<string, number>();
  const stub = (comp: string) => {
    let id = stubOf.get(comp);
    if (id === undefined) {
      id = nodes.length;
      const name = arch.components.find((c) => c.id === comp)?.name ?? comp;
      // A path with no folder, so the tree puts the stub at the top level.
      nodes.push({
        id,
        name,
        kind: "module",
        status: "caller",
        path: `\u0001${id}`,
        file: null,
        line: 0,
        rows: [],
        parent: null,
        hop: 0,
      });
      stubOf.set(comp, id);
      stubs.set(id, comp);
    }
    return id;
  };
  const edges = new Map<string, GraphEdge>();
  const end = (n: number) => {
    const own = ids.get(n);
    if (own !== undefined) return own;
    const comp = compOf.get(graph.nodes[n].path);
    return comp === undefined ? undefined : stub(comp);
  };
  for (const e of graph.edges) {
    if (!ids.has(e.from) && !ids.has(e.to)) continue;
    const [from, to] = [end(e.from), end(e.to)];
    if (from === undefined || to === undefined || from === to) continue;
    edges.set(`${from}>${to}`, { from, to });
  }
  for (const it of arch.intents) {
    if (it.kind === "move" && moveSource(arch, it) === component) stub(it.to);
    if (it.kind === "wrong" && it.from === component) stub(it.to);
  }
  return { graph: { nodes, edges: [...edges.values()] }, stubs };
}

/** The drill-down's tree, with each stub a box of its own at the top level rather than in a
 * file. */
export function scopedTree(scoped: Scoped): Tree {
  const tree = buildTree(scoped.graph);
  for (const id of scoped.stubs.keys()) {
    const box = tree.boxes.get(tree.boxOf[id]);
    if (!box?.parent) continue;
    const file = box.parent;
    tree.boxes.delete(file);
    box.parent = undefined;
    tree.roots = tree.roots.map((r) => (r === file ? box.id : r));
  }
  return tree;
}
