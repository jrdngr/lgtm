import type { SystemDto } from "../bindings";
import type { EdgeMode } from "./SystemBar";
import { type SEdge, type SNode, SYSTEM } from "./scene";

/** With nothing selected, Focus shows described relations and this many of the heaviest
 * imports. */
const HEAVIEST = 12;

/** The edges a level draws in `mode`. Changed draws the review's edges and those of the
 * `changed` nodes. */
export function visibleEdges(
  edges: SEdge[],
  mode: EdgeMode,
  selected?: string,
  changed?: Set<string>,
): SEdge[] {
  if (mode === "changed") {
    return edges.filter((e) => e.status || changed?.has(e.from) || changed?.has(e.to));
  }
  if (mode === "all") return edges;
  if (selected) return edges.filter((e) => e.from === selected || e.to === selected);
  const heavy = edges.filter((e) => !e.label).sort((a, b) => b.imports - a.imports);
  const keep = new Set(heavy.slice(0, HEAVIEST));
  return edges.filter((e) => e.label || keep.has(e));
}

/** `id` and the nodes its edges join it to. */
export function neighborhood(edges: SEdge[], id: string): Set<string> {
  const out = new Set([id]);
  for (const e of edges) {
    if (e.from === id) out.add(e.to);
    if (e.to === id) out.add(e.from);
  }
  return out;
}

/** What Ask sends a terminal agent about `node`. */
export function askPrompt(map: SystemDto, node: SNode): string {
  const what =
    node.id === SYSTEM
      ? `the ${map.repo} repository as a whole: its parts and how they fit together`
      : node.kind === "container"
        ? `the ${node.name} container${node.path ? ` (${node.path})` : ""} in this repository: what it does, its main parts and how it talks to the rest`
        : node.kind === "component"
          ? `the ${node.name} component (${node.path}) in this repository: what it does, its main files and how the rest uses it`
          : node.kind === "external"
            ? `how this repository uses ${node.name}`
            : `how ${node.name} uses this repository`;
  return `Explain ${what}. Keep it short and point me at the files to read first.`;
}
