import type { Canvas as Stored, ViewCanvas } from "../bindings";

/** A view's drawing with every field present. */
export type View = Required<ViewCanvas>;
/** A canvas with every field present. */
export type Canvas = Required<Omit<Stored, "views">> & { views: Record<string, View> };

const fullView = (v: ViewCanvas | undefined): View => ({
  frozen: v?.frozen ?? false,
  positions: v?.positions ?? {},
  shapes: v?.shapes ?? [],
  groups: v?.groups ?? [],
  edges: v?.edges ?? {},
  notes: v?.notes ?? [],
  arrows: v?.arrows ?? [],
  tags: v?.tags ?? {},
  agentTags: v?.agentTags ?? {},
});

/** `c` as stored, with its defaults filled in. */
export function fullCanvas(c: Stored | undefined): Canvas {
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

/** A shape, note, arrow or group id not yet taken in `v`. */
export function freshId(v: View, prefix: string): string {
  const taken = new Set([...v.shapes, ...v.groups, ...v.notes, ...v.arrows].map((x) => x.id));
  for (let n = 1; ; n++) if (!taken.has(`${prefix}${n}`)) return `${prefix}${n}`;
}

/** A part or shape id has a `:` or is `sys`, the system; a group id is neither. */
export const isPart = (id: string) => id.includes(":") || id === "sys";
export const isShape = (id: string) => id.startsWith("s:");
