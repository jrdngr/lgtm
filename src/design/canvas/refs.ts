/** What can be selected on a canvas: parts by their ids, and groups, notes, arrows and edges
 * by a prefixed id. */
export type Ref =
  | { kind: "part"; id: string }
  | { kind: "group"; id: string }
  | { kind: "note"; id: string }
  | { kind: "arrow"; id: string }
  | { kind: "edge"; id: string };

const PREFIX = { group: "g/", note: "n/", arrow: "a/", edge: "e/" } as const;

export const groupRef = (id: string) => `${PREFIX.group}${id}`;
export const noteRef = (id: string) => `${PREFIX.note}${id}`;
export const arrowRef = (id: string) => `${PREFIX.arrow}${id}`;
export const edgeRef = (key: string) => `${PREFIX.edge}${key}`;

export function parseRef(r: string): Ref {
  for (const [kind, p] of Object.entries(PREFIX) as [keyof typeof PREFIX, string][])
    if (r.startsWith(p)) return { kind, id: r.slice(p.length) };
  return { kind: "part", id: r };
}

/** The ids of the selected things of `kind`. */
export const idsOf = (sel: Set<string>, kind: Ref["kind"]) =>
  [...sel].map(parseRef).flatMap((r) => (r.kind === kind ? [r.id] : []));
