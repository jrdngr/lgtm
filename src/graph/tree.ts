import type { GraphDto, GraphNode } from "../bindings";

export type BoxKind = "folder" | "file" | "group" | "symbol";

/** One box in the drill-down: a folder, a file, a group of members under their enclosing
 * symbol, or a symbol. */
export interface Box {
  id: string;
  kind: BoxKind;
  label: string;
  parent?: string;
  children: string[];
  /** The graph nodes the box itself stands for: a symbol's node, or a group's own symbol
   * (the class whose members it holds) when that changed too. */
  own: number[];
  /** The file's path, for files and the boxes in them. */
  path?: string;
}

export interface Tree {
  boxes: Map<string, Box>;
  roots: string[];
  /** The box each graph node is, by node id. */
  boxOf: string[];
}

const fileId = (path: string) => `f:${path}`;
const groupId = (path: string, name: string) => `g:${path}#${name}`;

/** Folders hold folders and files, files hold their top-level symbols and groups, and a group
 * holds the members of one enclosing symbol. A folder with only one folder in it merges with
 * it, like `src/review`. */
export function buildTree(graph: GraphDto): Tree {
  const boxes = new Map<string, Box>();
  const add = (box: Omit<Box, "children" | "own">) => {
    let b = boxes.get(box.id);
    if (!b) {
      b = { ...box, children: [], own: [] };
      boxes.set(b.id, b);
    }
    return b;
  };
  const parentNames = new Set(
    graph.nodes.flatMap((n) => (n.parent ? [`${n.path}#${n.parent}`] : [])),
  );
  const isGroupOwner = (n: GraphNode) => parentNames.has(`${n.path}#${n.name}`);

  for (const n of graph.nodes) {
    add({ id: fileId(n.path), kind: "file", label: n.path, path: n.path });
    for (const name of [n.parent, isGroupOwner(n) ? n.name : null]) {
      if (name) add({ id: groupId(n.path, name), kind: "group", label: name, path: n.path });
    }
    const box = isGroupOwner(n)
      ? (boxes.get(groupId(n.path, n.name)) as Box)
      : add({ id: `s:${n.id}`, kind: "symbol", label: n.name, path: n.path });
    box.own.push(n.id);
  }
  const boxOf = graph.nodes.map((n) => (isGroupOwner(n) ? groupId(n.path, n.name) : `s:${n.id}`));

  // Parents: a symbol sits in its enclosing symbol's group, a group in its own symbol's
  // enclosing group, and everything else in its file.
  const enclosing = (b: Box): string => {
    const parent = b.own.map((id) => graph.nodes[id].parent).find((p) => p && p !== b.label);
    return parent ? groupId(b.path as string, parent) : fileId(b.path as string);
  };
  for (const b of boxes.values()) {
    if (b.kind === "symbol" || b.kind === "group") b.parent = enclosing(b);
  }
  // Break any loop of groups enclosing each other by putting the group in its file.
  for (const b of boxes.values()) {
    const seen = new Set<string>();
    for (let at: Box | undefined = b; at?.kind === "group"; at = boxes.get(at.parent ?? "")) {
      if (seen.has(at.id)) {
        b.parent = fileId(b.path as string);
        break;
      }
      seen.add(at.id);
    }
  }

  const roots = folders(boxes);
  for (const b of boxes.values()) {
    if (b.parent) boxes.get(b.parent)?.children.push(b.id);
  }
  const line = (id: string) => {
    const b = boxes.get(id) as Box;
    const own = b.own.map((n) => graph.nodes[n].line);
    return own.length > 0 ? Math.min(...own) : Number.MAX_SAFE_INTEGER;
  };
  const order = (a: string, b: string) => {
    const [x, y] = [boxes.get(a) as Box, boxes.get(b) as Box];
    const rank = (k: BoxKind) => (k === "folder" ? 0 : k === "file" ? 1 : 2);
    return rank(x.kind) - rank(y.kind) || line(a) - line(b) || x.label.localeCompare(y.label);
  };
  for (const b of boxes.values()) b.children.sort(order);
  roots.sort(order);
  return { boxes, roots, boxOf };
}

/** Adds folder boxes above the files and returns the top-level boxes. */
function folders(boxes: Map<string, Box>): string[] {
  const files = [...boxes.values()].filter((b) => b.kind === "file");
  // Folder paths with the folders and files directly in them.
  const kids = new Map<string, Set<string>>([["", new Set()]]);
  for (const f of files) {
    const parts = (f.path as string).split("/");
    let dir = "";
    for (const part of parts.slice(0, -1)) {
      const next = dir ? `${dir}/${part}` : part;
      if (!kids.has(next)) kids.set(next, new Set());
      kids.get(dir)?.add(`d:${next}`);
      dir = next;
    }
    kids.get(dir)?.add(f.id);
  }
  const roots: string[] = [];
  const place = (dir: string, parent: string | undefined, label: string) => {
    const inside = [...(kids.get(dir) ?? [])];
    // Merge a folder that holds nothing but one folder.
    if (dir && inside.length === 1 && inside[0].startsWith("d:")) {
      const only = inside[0].slice(2);
      return place(only, parent, `${label}/${only.slice(dir.length + 1)}`);
    }
    let here = parent;
    if (dir) {
      const id = `d:${dir}`;
      boxes.set(id, { id, kind: "folder", label, parent, children: [], own: [] });
      if (!parent) roots.push(id);
      here = id;
    }
    for (const k of inside) {
      if (k.startsWith("d:")) place(k.slice(2), here, k.slice(2 + (dir ? dir.length + 1 : 0)));
      else {
        const f = boxes.get(k) as Box;
        f.parent = here;
        f.label = here ? (f.path as string).slice(dir.length + 1) : (f.path as string);
        if (!here) roots.push(k);
      }
    }
  };
  place("", undefined, "");
  return roots;
}
