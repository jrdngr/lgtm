import type { SysComponentDto, SystemDto } from "../bindings";
import type { Lens } from "./lens";
import type { Row, Scene, SEdge, SNode } from "./scene";
import {
  add,
  componentId,
  containerColors,
  containerId,
  containersOf,
  edgeOf,
  wrap,
} from "./sceneParts";

/** Each component's tier by imports, 0 to 2: mostly importing puts it at the top, mostly
 * imported at the bottom. Cycles don't change it. */
export function computedTiers(
  ids: string[],
  edges: { from: string; to: string; imports: number }[],
) {
  const known = new Set(ids);
  const out = new Map<string, number>();
  const into = new Map<string, number>();
  for (const e of edges) {
    if (e.from === e.to || !known.has(e.from) || !known.has(e.to)) continue;
    out.set(e.from, (out.get(e.from) ?? 0) + e.imports);
    into.set(e.to, (into.get(e.to) ?? 0) + e.imports);
  }
  return new Map(
    ids.map((id) => {
      const o = out.get(id) ?? 0;
      const i = into.get(id) ?? 0;
      const share = o + i ? o / (o + i) : 0.5;
      return [id, share >= 2 / 3 ? 0 : share <= 1 / 3 ? 2 : 1];
    }),
  );
}

/** One container's components in tiers, with doors to the containers they import across. */
export function componentsScene(map: SystemDto, container: string, lens?: Lens): Scene {
  const colors = containerColors(map);
  const color = colors.get(container);
  const desc = new Map((map.description?.components ?? []).map((c) => [c.id, c]));
  const comps = map.components.filter((k) => k.container === container);
  const removed = (lens?.removed ?? []).filter(
    (k) => k.container === container && !comps.some((c) => c.id === k.id),
  );
  const node = (k: SysComponentDto): SNode => {
    const s = lens?.status.get(k.id);
    return {
      id: componentId(k.id),
      kind: "component",
      name: k.name,
      path: k.id,
      description: desc.get(k.id)?.description,
      color,
      status: s === "unchanged" ? undefined : s,
      delta: lens?.delta.get(k.id),
      flag: lens?.look.has(k.id),
    };
  };
  const nodes = comps.map(node);
  const tiers = map.description?.tiers ?? [];
  const described = comps.some((k) => desc.get(k.id)?.tier);
  let inside: Row[];
  if (described) {
    const tierOf = (n: SNode) => desc.get(n.id.slice(2))?.tier ?? undefined;
    inside = [
      ...tiers.flatMap((t) =>
        wrap(
          nodes.filter((n) => tierOf(n) === t),
          t,
        ),
      ),
      ...wrap(
        nodes.filter((n) => !tierOf(n)),
        "Other",
      ),
    ];
  } else {
    const ranks = computedTiers(
      comps.map((k) => k.id),
      map.edges,
    );
    inside = [0, 1, 2].flatMap((r) => wrap(nodes.filter((n) => ranks.get(n.id.slice(2)) === r)));
  }
  inside.push(...wrap(removed.map(node), removed.length ? "Removed" : undefined));
  const ids = new Set([...comps, ...removed].map((k) => k.id));
  const of = new Map(map.components.map((k) => [k.id, k.container]));
  const edges = new Map<string, SEdge>();
  const status = new Map((lens?.edges ?? []).map((e) => [edgeOf(e), e.status]));
  const doors = new Set<string>();
  for (const e of map.edges) {
    const inFrom = ids.has(e.from);
    const inTo = ids.has(e.to);
    if (inFrom && inTo) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: e.imports,
        status: status.get(`${e.from}>${e.to}`) === "added" ? "added" : undefined,
      });
    } else if (inFrom || inTo) {
      const other = of.get(inFrom ? e.to : e.from);
      if (other === undefined) continue;
      doors.add(other);
      const door = containerId(other);
      add(edges, {
        from: inFrom ? componentId(e.from) : door,
        to: inFrom ? door : componentId(e.to),
        imports: e.imports,
      });
    }
  }
  for (const e of lens?.edges ?? []) {
    if (e.status === "removed" && ids.has(e.from) && ids.has(e.to)) {
      add(edges, {
        from: componentId(e.from),
        to: componentId(e.to),
        imports: e.imports,
        status: "removed",
      });
    }
  }
  const containers = containersOf(map);
  const name = containers.find((c) => c.id === container)?.name ?? container;
  return {
    left: [],
    inside: inside.filter((r) => r.nodes.length),
    right: containers
      .filter((c) => doors.has(c.id))
      .map((c) => ({
        id: containerId(c.id),
        kind: "container",
        name: c.name,
        path: c.id,
        color: colors.get(c.id),
        door: true,
      })),
    boundary: name,
    color,
    edges: [...edges.values()],
  };
}
