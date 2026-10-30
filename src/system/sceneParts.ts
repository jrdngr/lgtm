import type { SystemDto } from "../bindings";
import { LOOSE, PALETTE } from "./colors";
import type { Row, SEdge, SNode } from "./scene";

export const personId = (name: string) => `p:${name}`;
export const externalId = (name: string) => `x:${name}`;
export const containerId = (id: string) => `c:${id}`;
export const componentId = (id: string) => `k:${id}`;
export const SYSTEM = "sys";

/** Past this many nodes a row wraps. */
export const PER_ROW = 6;

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

export function wrap(nodes: SNode[], label?: string, per = PER_ROW): Row[] {
  const rows: Row[] = [];
  for (let i = 0; i < nodes.length; i += per) {
    rows.push({ label: i === 0 ? label : undefined, nodes: nodes.slice(i, i + per) });
  }
  return rows;
}

/** Adds `e` to `edges`, merging it into an edge between the same nodes. */
export function add(edges: Map<string, SEdge>, e: SEdge) {
  const key = `${e.from}>${e.to}`;
  const had = edges.get(key);
  if (!had) return edges.set(key, e);
  had.imports += e.imports;
  if (e.label && !had.label?.split(" · ").includes(e.label)) {
    had.label = had.label ? `${had.label} · ${e.label}` : e.label;
  }
}

export const edgeOf = (e: { from: string; to: string }) => `${e.from}>${e.to}`;
