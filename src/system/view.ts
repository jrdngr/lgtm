import type { SystemDto } from "../bindings";
import { type SEdge, type SNode, SYSTEM } from "./scene";

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
      : node.kind === "group"
        ? `the ${node.name} group of containers in this repository: what they do together and how they talk to the rest`
        : node.kind === "container"
          ? `the ${node.name} container${node.path ? ` (${node.path})` : ""} in this repository: what it does, its main parts and how it talks to the rest`
          : node.kind === "component"
            ? `the ${node.name} component (${node.path}) in this repository: what it does, its main files and how the rest uses it`
            : node.kind === "external"
              ? `how this repository uses ${node.name}`
              : `how ${node.name} uses this repository`;
  return `Explain ${what}. Keep it short and point me at the files to read first.`;
}
