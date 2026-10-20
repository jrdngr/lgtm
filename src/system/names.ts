import type { SystemDto } from "../bindings";
import { NEUTRAL } from "./colors";
import { containersOf, SYSTEM } from "./scene";

export interface NodeInfo {
  name: string;
  /** A container's color, or its container's for a component. */
  color: string;
  kind: "system" | "person" | "external" | "container" | "component" | "group";
  /** The container a component is in. */
  container?: string;
}

/** What to call any node id, and its color. */
export function nodeInfo(
  map: SystemDto,
  colors: Map<string, string>,
  id: string,
): NodeInfo | undefined {
  if (id === SYSTEM) return { name: map.repo, color: NEUTRAL, kind: "system" };
  const ref = id.slice(2);
  if (id.startsWith("p:")) return { name: ref, color: NEUTRAL, kind: "person" };
  if (id.startsWith("x:")) return { name: ref, color: NEUTRAL, kind: "external" };
  if (id.startsWith("g:")) return { name: ref, color: NEUTRAL, kind: "group" };
  if (id.startsWith("c:")) {
    const c = containersOf(map).find((c) => c.id === ref);
    return c && { name: c.name, color: colors.get(c.id) ?? NEUTRAL, kind: "container" };
  }
  const k = map.components.find((c) => c.id === ref);
  return (
    k && {
      name: k.name,
      color: colors.get(k.container) ?? NEUTRAL,
      kind: "component",
      container: k.container,
    }
  );
}
