import type { CanvasGroup } from "../bindings";
import type { Box } from "../system/layout";
import { isPart, type View } from "./view";

/** Around a group's members, and above them for its label. */
const PAD = 16;
const TOP = 28;

export type GroupBox = Box & { depth: number };

export const parentOf = (v: View, member: string) =>
  v.groups.find((g) => g.members.includes(member))?.id;

/** How deep group `id` sits: 0 at the top. */
function depthOf(v: View, id: string): number {
  const p = parentOf(v, id);
  return p ? depthOf(v, p) + 1 : 0;
}

/** Every part inside group `id`, at any depth. */
export function partsIn(v: View, id: string): string[] {
  const g = v.groups.find((x) => x.id === id);
  if (!g) return [];
  return g.members.flatMap((m) => (isPart(m) ? [m] : partsIn(v, m)));
}

/** Group `id` and every group inside it. */
export function groupsIn(v: View, id: string): string[] {
  const g = v.groups.find((x) => x.id === id);
  if (!g) return [];
  return [id, ...g.members.filter((m) => !isPart(m)).flatMap((m) => groupsIn(v, m))];
}

function union(boxes: Box[]): Box | undefined {
  if (!boxes.length) return undefined;
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  const r = Math.max(...boxes.map((b) => b.x + b.w));
  const bottom = Math.max(...boxes.map((b) => b.y + b.h));
  return { x, y, w: r - x, h: bottom - y };
}

/** Each group's frame around its members' boxes, leaving out the members in `skip`. A group
 * with no members on the map has none. */
export function groupBoxes(
  v: View,
  parts: Map<string, Box>,
  skip: Set<string> = new Set(),
): Map<string, GroupBox> {
  const out = new Map<string, GroupBox>();
  const boxOf = (id: string, seen: Set<string>): Box | undefined => {
    if (out.has(id)) return out.get(id);
    const g = v.groups.find((x) => x.id === id);
    if (!g || seen.has(id)) return undefined;
    seen.add(id);
    const inner = g.members
      .filter((m) => !skip.has(m))
      .map((m) => (isPart(m) ? parts.get(m) : boxOf(m, seen)))
      .filter((b): b is Box => !!b);
    const u = union(inner);
    if (!u) return undefined;
    const box = {
      x: u.x - PAD,
      y: u.y - TOP,
      w: u.w + PAD * 2,
      h: u.h + PAD + TOP,
      depth: depthOf(v, id),
    };
    out.set(id, box);
    return box;
  };
  for (const g of v.groups) boxOf(g.id, new Set());
  return out;
}

/** The innermost group whose frame holds point (`x`, `y`), other than those in `except`. */
export function groupAt(
  boxes: Map<string, GroupBox>,
  x: number,
  y: number,
  except: Set<string>,
): string | undefined {
  let best: [string, number] | undefined;
  for (const [id, b] of boxes) {
    if (except.has(id) || x < b.x || x > b.x + b.w || y < b.y || y > b.y + b.h) continue;
    if (!best || b.depth > best[1]) best = [id, b.depth];
  }
  return best?.[0];
}

const dropEmpty = (groups: CanvasGroup[]) => {
  let out = groups;
  for (;;) {
    const empty = new Set(out.filter((g) => !g.members.length).map((g) => g.id));
    if (!empty.size) return out;
    out = out
      .filter((g) => !empty.has(g.id))
      .map((g) => ({ ...g, members: g.members.filter((m) => !empty.has(m)) }));
  }
};

/** `v` with `items` (parts and groups) moved into group `to`, or out of every group. */
export function moveInto(v: View, items: string[], to: string | undefined): View {
  const moving = new Set(items.filter((i) => !to || !groupsIn(v, i).includes(to)));
  if (!moving.size) return v;
  const groups = v.groups.map((g) => {
    const kept = g.members.filter((m) => !moving.has(m));
    return { ...g, members: g.id === to ? [...kept, ...moving] : kept };
  });
  return { ...v, groups: dropEmpty(groups) };
}

/** `v` with `items` wrapped in a new group `id`, which takes the place of the first of them in
 * their shared parent, if they share one. */
export function group(v: View, items: string[], id: string, label: string): View {
  const parents = new Set(items.map((i) => parentOf(v, i)));
  const parent = parents.size === 1 ? [...parents][0] : undefined;
  const taken = new Set(items);
  let groups = v.groups.map((g) => {
    if (g.id !== parent) return { ...g, members: g.members.filter((m) => !taken.has(m)) };
    const at = g.members.findIndex((m) => taken.has(m));
    const kept = g.members.filter((m) => !taken.has(m));
    kept.splice(Math.min(at, kept.length), 0, id);
    return { ...g, members: kept };
  });
  groups = [...groups, { id, label, color: null, members: items, by: "user" }];
  return { ...v, groups: dropEmpty(groups) };
}

/** `v` without group `id`; its members take its place in its parent, if it had one. */
export function ungroup(v: View, id: string): View {
  const g = v.groups.find((x) => x.id === id);
  if (!g) return v;
  const groups = v.groups
    .filter((x) => x.id !== id)
    .map((x) => {
      const at = x.members.indexOf(id);
      if (at < 0) return x;
      const members = [...x.members];
      members.splice(at, 1, ...g.members);
      return { ...x, members };
    });
  return { ...v, groups };
}
