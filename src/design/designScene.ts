import type {
  DesignBuilt,
  DesignChange,
  DesignComponentDto,
  DesignContainerDto,
  DesignMapDto,
  SpecRefDto,
  SystemDto,
} from "../bindings";
import type { Mark, Scene, SEdge, SNode } from "../system/scene";
import { containerColors } from "../system/scene";

/** The levels a design map has. */
export type DesignLevel =
  | { kind: "context" }
  | { kind: "containers" }
  | { kind: "components"; container: string }
  | { kind: "interfaces"; component: string };

/** What a design node stands for, for its detail panel. */
export type DesignNode =
  | { kind: "person"; name: string; description: string }
  | { kind: "external"; name: string; tech: string; description: string }
  | { kind: "container"; c: DesignContainerDto }
  | { kind: "component"; k: DesignComponentDto }
  | { kind: "interface"; k: DesignComponentDto; at: number }
  | { kind: "code"; id: string; name: string };

const PER_ROW = 3;
const COMPONENTS_PER_ROW = 4;

export const designSystemId = "sys";
const pid = (name: string) => `p:${name}`;
const xid = (name: string) => `x:${name}`;
export const dcid = (id: string) => `c:${id}`;
export const dkid = (id: string) => `k:${id}`;
const iid = (component: string, at: number) => `i:${component}#${at}`;

/** Whether the code has been compared with the design: some part is built or differs. */
export const hasDrift = (m: DesignMapDto) =>
  [...m.containers, ...m.components, ...m.externals].some(
    (n) => n.built === "built" || n.built === "differs",
  );

/** How a node is marked: by what the design changes, or once code lands, by whether it's
 * built. Nothing when the design alone shows, or for what it keeps as is. */
function markOf(
  change: DesignChange,
  built: DesignBuilt | null,
  drift: boolean,
  overCode: boolean,
): Mark | undefined {
  if (!overCode) return undefined;
  if (drift && change !== "removed" && built) return built;
  if (change === "same") return undefined;
  return change;
}

function rows(nodes: SNode[], per: number) {
  const out = [];
  for (let i = 0; i < nodes.length; i += per) out.push({ nodes: nodes.slice(i, i + per) });
  return out;
}

/** The code containers no design container stands for. */
function codeOnly(m: DesignMapDto, code: SystemDto) {
  const taken = new Set(m.containers.flatMap((c) => [c.id, ...(c.code ? [c.code] : [])]));
  return code.containers.filter((c) => !taken.has(c.id));
}

/** Relations and, over code, the code's imports, between the nodes `at` maps endpoints to.
 * Joins labels of relations that land on the same pair. */
function edgesBetween(
  m: DesignMapDto,
  code: SystemDto | undefined,
  at: (end: string) => string | undefined,
): SEdge[] {
  const out = new Map<string, SEdge>();
  for (const r of m.relations) {
    const from = at(r.from);
    const to = at(r.to);
    if (!from || !to || from === to) continue;
    const key = `${from}>${to}`;
    const status = r.change === "new" ? "added" : r.change === "removed" ? "removed" : undefined;
    const e = out.get(key);
    if (!e) out.set(key, { from, to, imports: 0, label: r.label, status });
    else if (r.label && !e.label?.includes(r.label))
      e.label = e.label ? `${e.label} · ${r.label}` : r.label;
  }
  if (code) {
    const containerOf = new Map(code.components.map((k) => [k.id, k.container]));
    for (const e of code.edges) {
      const from = at(`code:${containerOf.get(e.from)}`);
      const to = at(`code:${containerOf.get(e.to)}`);
      if (!from || !to || from === to) continue;
      const key = `${from}>${to}`;
      const had = out.get(key);
      if (had) had.imports += e.imports;
      else out.set(key, { from, to, imports: e.imports });
    }
  }
  return [...out.values()];
}

/** The design's people and external systems as side nodes. */
function sides(m: DesignMapDto, drift: boolean, overCode: boolean) {
  const left: SNode[] = m.people.map((p) => ({
    id: pid(p.name),
    kind: "person",
    name: p.name,
    description: p.description,
    mark: markOf(p.change, null, drift, overCode),
  }));
  const right: SNode[] = m.externals.map((x) => ({
    id: xid(x.name),
    kind: "external",
    name: x.name,
    tech: x.kind,
    description: x.description,
    mark: markOf(x.change, x.built, drift, overCode),
  }));
  return { left, right };
}

/** The design as a scene at `level`, laid over the code's map when `overCode`. */
export function designScene(
  m: DesignMapDto,
  name: string,
  code: SystemDto | undefined,
  level: DesignLevel,
  overCode: boolean,
): Scene {
  const drift = overCode && hasDrift(m);
  const over = overCode ? code : undefined;
  const colors = code ? containerColors(code) : new Map<string, string>();
  const containerNode = (c: DesignContainerDto): SNode => ({
    id: dcid(c.id),
    kind: "container",
    name: c.name,
    tech: c.tech,
    description: c.description,
    path: c.code ?? undefined,
    color: c.code ? colors.get(c.code) : undefined,
    mark: markOf(c.change, c.built, drift, overCode),
  });
  const containerOfComponent = new Map(m.components.map((k) => [k.id, k.container]));
  const designOfCode = new Map(m.containers.flatMap((c) => (c.code ? [[c.code, c.id]] : [])));
  const named = new Set([...m.people.map((p) => p.name), ...m.externals.map((x) => x.name)]);
  const outside = (end: string) =>
    named.has(end) ? (m.people.some((p) => p.name === end) ? pid(end) : xid(end)) : undefined;
  const { left, right } = sides(m, drift, overCode);
  const extra = over ? codeOnly(m, over) : [];

  if (level.kind === "context") {
    const at = (end: string) => outside(end) ?? designSystemId;
    return {
      left,
      inside: [
        {
          nodes: [
            {
              id: designSystemId,
              kind: "system",
              name,
              description: m.summary,
              blocks: m.containers.map((c) => ({
                name: c.name,
                color: (c.code && colors.get(c.code)) || "#a371f7",
                files: 0,
              })),
            },
          ],
        },
      ],
      right,
      edges: edgesBetween(m, undefined, at),
    };
  }

  if (level.kind === "containers") {
    const at = (end: string) => {
      if (end.startsWith("code:")) {
        const id = end.slice(5);
        const d = designOfCode.get(id);
        return d ? dcid(d) : extra.some((c) => c.id === id) ? dcid(id) : undefined;
      }
      const container = containerOfComponent.get(end) ?? end;
      if (m.containers.some((c) => c.id === container)) return dcid(container);
      return outside(end);
    };
    const nodes = [
      ...m.containers.map(containerNode),
      ...extra.map(
        (c): SNode => ({
          id: dcid(c.id),
          kind: "container",
          name: c.name,
          path: c.id,
          color: colors.get(c.id),
        }),
      ),
    ];
    return {
      left,
      inside: rows(nodes, PER_ROW),
      right,
      boundary: name,
      edges: edgesBetween(m, over, at),
    };
  }

  if (level.kind === "components") {
    const container = m.containers.find((c) => c.id === level.container);
    const mine = m.components.filter((k) => k.container === level.container);
    const at = (end: string) => {
      if (mine.some((k) => k.id === end)) return dkid(end);
      const c = containerOfComponent.get(end) ?? end;
      if (c !== level.container && m.containers.some((x) => x.id === c)) return dcid(c);
      return outside(end);
    };
    const nodes = mine.map(
      (k): SNode => ({
        id: dkid(k.id),
        kind: "component",
        name: k.name,
        description: k.description,
        path: k.code ?? undefined,
        color: container?.code ? colors.get(container.code) : undefined,
        mark: markOf(k.change, k.built, drift, overCode),
        lines: k.interfaces.map((i) => i.signature || i.name),
      }),
    );
    const edges = edgesBetween(m, undefined, at);
    const wired = new Set(edges.flatMap((e) => [e.from, e.to]));
    const doors = m.containers
      .filter((c) => c.id !== level.container && wired.has(dcid(c.id)))
      .map((c) => ({ ...containerNode(c), door: true }));
    return {
      left: left.filter((n) => wired.has(n.id)),
      inside: rows(nodes, COMPONENTS_PER_ROW),
      right: [...doors, ...right.filter((n) => wired.has(n.id))],
      boundary: container?.name,
      edges,
    };
  }

  const k = m.components.find((x) => x.id === level.component);
  if (!k) return { left: [], inside: [], right: [], edges: [] };
  const nodes = k.interfaces.map(
    (i, n): SNode => ({
      id: iid(k.id, n),
      kind: "component",
      name: i.signature || i.name,
      description: i.description,
      mark: markOf(k.change === "new" ? "new" : "same", i.built, drift, overCode),
    }),
  );
  const at = (end: string) =>
    end === k.id
      ? undefined
      : m.components.some((x) => x.id === end)
        ? dkid(end)
        : m.containers.some((c) => c.id === end)
          ? dcid(end)
          : outside(end);
  const users: SNode[] = [];
  const uses: SNode[] = [];
  for (const r of m.relations) {
    if (r.to === k.id && at(r.from)) users.push(sideNode(m, r.from, at(r.from) as string));
    if (r.from === k.id && at(r.to)) uses.push(sideNode(m, r.to, at(r.to) as string));
  }
  return {
    left: dedupe(users),
    inside: rows(nodes, 2),
    right: dedupe(uses),
    boundary: k.name,
    edges: [],
  };
}

function sideNode(m: DesignMapDto, end: string, id: string): SNode {
  const k = m.components.find((x) => x.id === end);
  if (k) return { id, kind: "component", name: k.name, door: true };
  const c = m.containers.find((x) => x.id === end);
  if (c) return { id, kind: "container", name: c.name, door: true };
  return { id, kind: id.startsWith("p:") ? "person" : "external", name: end };
}

const dedupe = (nodes: SNode[]) =>
  nodes.filter((n, i) => nodes.findIndex((x) => x.id === n.id) === i);

/** What scene node `id` stands for. */
export function designNode(m: DesignMapDto, code: SystemDto | undefined, id: string) {
  const [kind, rest] = [id.slice(0, 2), id.slice(2)];
  if (kind === "p:") {
    const p = m.people.find((x) => x.name === rest);
    return p && ({ kind: "person", name: p.name, description: p.description } as DesignNode);
  }
  if (kind === "x:") {
    const x = m.externals.find((e) => e.name === rest);
    return (
      x &&
      ({ kind: "external", name: x.name, tech: x.kind, description: x.description } as DesignNode)
    );
  }
  if (kind === "c:") {
    const c = m.containers.find((x) => x.id === rest);
    if (c) return { kind: "container", c } as DesignNode;
    const cc = code?.containers.find((x) => x.id === rest);
    return cc && ({ kind: "code", id: cc.id, name: cc.name } as DesignNode);
  }
  if (kind === "k:") {
    const k = m.components.find((x) => x.id === rest);
    return k && ({ kind: "component", k } as DesignNode);
  }
  if (kind === "i:") {
    const [comp, at] = [
      rest.slice(0, rest.lastIndexOf("#")),
      Number(rest.slice(rest.lastIndexOf("#") + 1)),
    ];
    const k = m.components.find((x) => x.id === comp);
    return k && ({ kind: "interface", k, at } as DesignNode);
  }
  return undefined;
}

/** The level a node opens: a container's components, a component's interfaces. */
export function openLevel(m: DesignMapDto, id: string): DesignLevel | undefined {
  if (id === designSystemId) return { kind: "containers" };
  if (id.startsWith("c:") && m.containers.some((c) => c.id === id.slice(2)))
    return { kind: "components", container: id.slice(2) };
  if (id.startsWith("k:") && m.components.some((k) => k.id === id.slice(2)))
    return { kind: "interfaces", component: id.slice(2) };
  return undefined;
}

/** Spec lines behind a node. */
export function specOf(n: DesignNode | undefined): SpecRefDto[] {
  if (!n) return [];
  if (n.kind === "container") return n.c.spec;
  if (n.kind === "component") return n.k.spec;
  if (n.kind === "interface") return n.k.interfaces[n.at]?.spec ?? [];
  return [];
}
