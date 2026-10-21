import type { DesignCanvas, ViewCanvas } from "../../bindings";
import type { DesignLevel } from "../designScene";

/** A view's drawing with every field present. */
export type View = Required<ViewCanvas>;
/** A design's canvas with every field present. */
export type Canvas = Required<Omit<DesignCanvas, "views">> & { views: Record<string, View> };

/** The canvas key of a design level: its kind, and the container or component it shows. */
export function viewKey(level: DesignLevel): string {
  if (level.kind === "components") return `components/${level.container}`;
  if (level.kind === "interfaces") return `interfaces/${level.component}`;
  return level.kind;
}

/** The design level a canvas key draws. */
export function levelOfKey(key: string): DesignLevel {
  const [kind, id = ""] = key.split(/\/(.*)/s);
  if (kind === "components") return { kind, container: id };
  if (kind === "interfaces") return { kind, component: id };
  return { kind: kind === "context" ? "context" : "containers" };
}

const fullView = (v: ViewCanvas | undefined): View => ({
  frozen: v?.frozen ?? false,
  positions: v?.positions ?? {},
  groups: v?.groups ?? [],
  edges: v?.edges ?? {},
  notes: v?.notes ?? [],
  arrows: v?.arrows ?? [],
});

/** `c` as stored, with its defaults filled in. */
export function fullCanvas(c: DesignCanvas | undefined): Canvas {
  return {
    views: Object.fromEntries(Object.entries(c?.views ?? {}).map(([k, v]) => [k, fullView(v)])),
    colors: c?.colors ?? {},
    agent: c?.agent ?? null,
  };
}

export const viewOf = (c: Canvas, key: string): View => c.views[key] ?? fullView(undefined);

/** `c` with view `key` changed by `f`. */
export function withView(c: Canvas, key: string, f: (v: View) => View): Canvas {
  return { ...c, views: { ...c.views, [key]: f(viewOf(c, key)) } };
}

/** A note, arrow or group id not yet taken in `v`. */
export function freshId(v: View, prefix: string): string {
  const taken = new Set([...v.groups, ...v.notes, ...v.arrows].map((x) => x.id));
  for (let n = 1; ; n++) if (!taken.has(`${prefix}${n}`)) return `${prefix}${n}`;
}

/** A part id has a `:` or is `sys`, the system; a group id is neither. */
export const isPart = (id: string) => id.includes(":") || id === "sys";
