import type { ArchDto, CompStatus, LookDto, RowDto, SysComponentDto } from "../bindings";

export interface Delta {
  add: number;
  del: number;
}

export interface ChangedFile extends Delta {
  path: string;
  /** The review file. */
  file: number;
}

/** What the review changes, by component id: the Review lens over the map. */
export interface Lens {
  status: Map<string, CompStatus>;
  /** Components only the base has. */
  removed: SysComponentDto[];
  /** Imports the review adds or removes between components, with their sites. */
  edges: { from: string; to: string; status: "added" | "removed"; imports: number }[];
  delta: Map<string, Delta>;
  files: Map<string, ChangedFile[]>;
  /** Open Look items by component, and by edge `from>to`. */
  look: Map<string, LookDto[]>;
  edgeLook: Map<string, LookDto[]>;
  /** Open Look items in all. */
  open: number;
}

/** Added and removed rows of a file. */
function fileDelta(rows: RowDto[]): Delta {
  let add = 0;
  let del = 0;
  for (const r of rows) {
    if (r.kind === "added") add++;
    else if (r.kind === "removed") del++;
  }
  return { add, del };
}

const push = <T>(m: Map<string, T[]>, k: string, v: T) => {
  const list = m.get(k);
  if (list) list.push(v);
  else m.set(k, [v]);
};

/** The lens of `arch`, whose files are the review's `files`. */
export function lensOf(arch: ArchDto, files: { rows: RowDto[] }[]): Lens {
  const status = new Map<string, CompStatus>();
  const removed: SysComponentDto[] = [];
  const delta = new Map<string, Delta>();
  const changed = new Map<string, ChangedFile[]>();
  for (const c of arch.components) {
    status.set(c.id, c.status);
    if (c.status === "removed") {
      removed.push({
        id: c.id,
        name: c.name,
        container: c.container,
        files: c.files.length,
        entry: [],
      });
    }
    const sum: Delta = { add: 0, del: 0 };
    for (const f of c.files) {
      const rows = f.file === null ? undefined : files[f.file]?.rows;
      if (!rows || f.file === null) continue;
      const d = fileDelta(rows);
      sum.add += d.add;
      sum.del += d.del;
      push(changed, c.id, { path: f.path, file: f.file, ...d });
    }
    if (sum.add || sum.del) delta.set(c.id, sum);
  }
  const edges = arch.edges.flatMap((e) =>
    e.status === "same"
      ? []
      : [{ from: e.from, to: e.to, status: e.status, imports: e.imports.length }],
  );
  const look = new Map<string, LookDto[]>();
  const edgeLook = new Map<string, LookDto[]>();
  const open = arch.look.filter((l) => !l.checked);
  for (const l of open) {
    if (l.edge) push(edgeLook, `${l.edge[0]}>${l.edge[1]}`, l);
    const comp = l.component ?? l.edge?.[0];
    if (comp) push(look, comp, l);
  }
  return { status, removed, edges, delta, files: changed, look, edgeLook, open: open.length };
}

/** Whether the review changes the component itself. */
export const isChanged = (s: CompStatus | undefined) =>
  s === "changed" || s === "added" || s === "removed";

export function addDelta(a: Delta | undefined, b: Delta | undefined): Delta | undefined {
  if (!a) return b;
  if (!b) return a;
  return { add: a.add + b.add, del: a.del + b.del };
}

/** `lens` narrowed to `paths`: only components with one of those files show as changed, with
 * only their deltas, wires and Look items. */
export function focusLens(lens: Lens, paths: readonly string[]): Lens {
  const keep = new Set(paths);
  const files = new Map<string, ChangedFile[]>();
  const delta = new Map<string, Delta>();
  for (const [id, list] of lens.files) {
    const mine = list.filter((f) => keep.has(f.path));
    if (!mine.length) continue;
    files.set(id, mine);
    const d = mine.reduce((sum, f) => ({ add: sum.add + f.add, del: sum.del + f.del }), {
      add: 0,
      del: 0,
    });
    if (d.add || d.del) delta.set(id, d);
  }
  const status = new Map<string, CompStatus>();
  for (const [id, s] of lens.status) status.set(id, files.has(id) ? s : "unchanged");
  const look = new Map([...lens.look].filter(([id]) => files.has(id)));
  const edgeLook = new Map(
    [...lens.edgeLook].filter(([key]) => key.split(">").some((id) => files.has(id))),
  );
  return {
    status,
    removed: [],
    edges: lens.edges.filter((e) => files.has(e.from) || files.has(e.to)),
    delta,
    files,
    look,
    edgeLook,
    open: [...look.values()].reduce((n, l) => n + l.length, 0),
  };
}
