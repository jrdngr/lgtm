import type { SystemDto, TourDto, TourStepDto } from "../bindings";
import { NEUTRAL } from "./colors";
import { nodeInfo } from "./names";
import { componentId, containerId, containersOf, type Level, SYSTEM } from "./scene";

/** A tour step, placed: the level it shows and what it picks there. */
export interface Stop {
  key: string;
  level: Level;
  /** The node it selects. */
  node?: string;
  /** The edge it selects, between node ids. */
  edge?: { from: string; to: string };
  /** At the Symbols level, the file it marks, or the symbol it selects. */
  file?: string;
  symbol?: { path: string; name: string; line: number | null };
  name: string;
  /** Names a file or a symbol. */
  mono: boolean;
  kind: string;
  color: string;
  text: string;
  /** 0 for context to 3 for symbols. */
  depth: number;
}

const DEPTH: Record<Level["kind"], number> = {
  context: 0,
  containers: 1,
  components: 2,
  symbols: 3,
};

/** A step's node as a map id: the system, a container or a component. */
function nodeId(map: SystemDto, ref: string): string | undefined {
  if (ref === "system") return SYSTEM;
  if (map.components.some((k) => k.id === ref)) return componentId(ref);
  if (containersOf(map).some((c) => c.id === ref)) return containerId(ref);
  return undefined;
}

/** The component a file is in. */
const componentOfFile = (map: SystemDto, path: string) =>
  map.files.find((f) => f.path === path)?.component;

const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** Where `step` of a tour of `subject` shows, or undefined when the map lost its node. */
export function stopOf(
  map: SystemDto,
  colors: Map<string, string>,
  subject: string,
  step: TourStepDto,
  i: number,
): Stop | undefined {
  const id = nodeId(map, step.node);
  if (!id) return undefined;
  const info = nodeInfo(map, colors, id);
  const color = info?.color ?? NEUTRAL;
  const at = { key: `${i}`, color, text: step.text, mono: false };
  const comp = step.path ? componentOfFile(map, step.path) : undefined;
  if (step.path && comp !== undefined) {
    const level: Level = { kind: "symbols", component: comp };
    return step.symbol
      ? {
          ...at,
          level,
          symbol: { path: step.path, name: step.symbol, line: step.line ?? null },
          name: step.symbol,
          mono: true,
          kind: "Symbol",
          depth: 3,
        }
      : {
          ...at,
          level,
          file: step.path,
          name: base(step.path),
          mono: true,
          kind: "File",
          depth: 3,
        };
  }
  const to = step.to ? nodeId(map, step.to) : undefined;
  // The context level draws no edges, so a step to or from the system shows its other end.
  if (to && to !== id && (id === SYSTEM || to === SYSTEM)) {
    const other = id === SYSTEM ? step.to : step.node;
    return stopOf(map, colors, subject, { ...step, node: other ?? step.node, to: null }, i);
  }
  if (to && to !== id) {
    const other = nodeInfo(map, colors, to);
    const level = edgeLevel(map, id, to);
    const kind = to === subject ? "Who uses it" : id === subject ? "What it uses" : "Uses";
    return {
      ...at,
      level,
      edge: { from: id, to },
      name: `${info?.name ?? step.node} → ${other?.name ?? step.to}`,
      kind,
      depth: DEPTH[level.kind],
    };
  }
  if (id === SYSTEM) {
    return {
      ...at,
      level: { kind: "context" },
      node: id,
      name: map.repo,
      kind: "Context",
      depth: 0,
    };
  }
  if (info?.kind === "container") {
    return {
      ...at,
      level: { kind: "containers" },
      node: id,
      name: info.name,
      kind: "Container",
      depth: 1,
    };
  }
  return {
    ...at,
    level: { kind: "components", container: info?.container ?? "" },
    node: id,
    name: info?.name ?? step.node,
    kind: "Component",
    depth: 2,
  };
}

/** The level that draws an edge: the components of `from`'s container when either end is a
 * component, else the containers. */
function edgeLevel(map: SystemDto, from: string, to: string): Level {
  const k = map.components.find((c) => componentId(c.id) === from || componentId(c.id) === to);
  const own = map.components.find((c) => componentId(c.id) === from);
  return k ? { kind: "components", container: (own ?? k).container } : { kind: "containers" };
}

/** The tour's steps the map still has nodes for. */
export const stopsOf = (map: SystemDto, colors: Map<string, string>, tour: TourDto): Stop[] =>
  tour.steps.flatMap((s, i) => stopOf(map, colors, tour.id, s, i) ?? []);

/** The tour of node `id`, if it has one. */
export const tourOf = (map: SystemDto, id: string) => map.tours.find((t) => t.id === id);

/** What a tour's row says it covers: its summary, or its first steps' names. */
export function tourPath(map: SystemDto, colors: Map<string, string>, tour: TourDto): string {
  if (tour.summary) return tour.summary;
  const names = stopsOf(map, colors, tour)
    .filter((s) => !s.edge)
    .map((s) => s.name);
  return [...new Set(names)].slice(0, 4).join(" › ");
}
