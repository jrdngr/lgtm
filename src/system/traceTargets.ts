import type { SystemDto } from "../bindings";
import { NEUTRAL } from "./colors";
import { componentId, containerId, containersOf, externalId, personId } from "./scene";

/** A node a trace can start or end at. */
export interface Target {
  id: string;
  name: string;
  color: string;
  kind: "container" | "component" | "person" | "external";
  /** The container a component is in. */
  where?: string;
}

const SHOWN = 8;

/** Every node at any level that a path can join: people, externals, containers, components. */
export function traceTargets(map: SystemDto, colors: Map<string, string>): Target[] {
  const d = map.description;
  const containers = containersOf(map);
  return [
    ...(d?.people ?? []).map(
      (p): Target => ({ id: personId(p.name), name: p.name, color: NEUTRAL, kind: "person" }),
    ),
    ...(d?.externals ?? []).map(
      (x): Target => ({ id: externalId(x.name), name: x.name, color: NEUTRAL, kind: "external" }),
    ),
    ...containers.map(
      (c): Target => ({
        id: containerId(c.id),
        name: c.name,
        color: colors.get(c.id) ?? NEUTRAL,
        kind: "container",
      }),
    ),
    ...map.components.map(
      (k): Target => ({
        id: componentId(k.id),
        name: k.name,
        color: colors.get(k.container) ?? NEUTRAL,
        kind: "component",
        where: containers.find((c) => c.id === k.container)?.name,
      }),
    ),
  ];
}

/** Targets whose name, or component path, holds `q`: names that start with it first, then the
 * shortest. */
export function matchTargets(all: Target[], q: string, skip?: string): Target[] {
  const query = q.trim().toLowerCase();
  if (!query) return [];
  const rank = (t: Target) => {
    const name = t.name.toLowerCase();
    if (name.startsWith(query)) return 0;
    if (name.includes(query)) return 1;
    return t.kind === "component" && t.id.toLowerCase().includes(query) ? 2 : -1;
  };
  return all
    .filter((t) => t.id !== skip)
    .map((t) => ({ t, r: rank(t) }))
    .filter(({ r }) => r >= 0)
    .sort((a, b) => a.r - b.r || a.t.name.length - b.t.name.length)
    .slice(0, SHOWN)
    .map(({ t }) => t);
}
