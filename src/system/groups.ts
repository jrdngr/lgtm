import type { SystemDto } from "../bindings";
import { containersOf } from "./scene";

/** Containers that belong together on a large system's maps. */
export interface Group {
  /** `g:` and its name. */
  id: string;
  name: string;
  description?: string;
  /** Container ids, in map order. */
  containers: string[];
}

export const groupId = (name: string) => `g:${name}`;

/** Names the group of containers the description left out. */
const OTHER = "Other";
/** Up to this many containers show without groups unless the description has some. */
const FEW = 5;

/** The system's groups: the description's, or else, past a few containers, each container's
 * parent folder. None when that makes fewer than two, since one group is the whole system, or
 * when the view turns groups off. */
export function groupsOf(map: SystemDto): Group[] | undefined {
  if (!map.grouped) return undefined;
  const ids = containersOf(map).map((c) => c.id);
  const described = map.description?.groups ?? [];
  const byName = new Map<string, string[]>();
  const put = (name: string, id: string) => {
    const list = byName.get(name);
    if (list) list.push(id);
    else byName.set(name, [id]);
  };
  if (described.length) {
    const of = new Map(described.flatMap((g) => g.containers.map((c) => [c, g.name])));
    for (const g of described) byName.set(g.name, []);
    for (const id of ids) put(of.get(id) ?? OTHER, id);
  } else if (ids.length > FEW) {
    for (const id of ids) put(parentName(map, id), id);
  }
  const out = [...byName]
    .filter(([, list]) => list.length)
    .map(([name, list]) => ({
      id: groupId(name),
      name,
      description: described.find((g) => g.name === name)?.description || undefined,
      containers: list,
    }));
  return out.length > 1 ? out : undefined;
}

/** A container's parent folder's name; the repo's for a top-level one or loose files. */
function parentName(map: SystemDto, id: string): string {
  const cut = id.lastIndexOf("/");
  if (cut < 0) return map.repo;
  const parent = id.slice(0, cut);
  return parent.slice(parent.lastIndexOf("/") + 1);
}

/** Each container's group id, by container id. */
export function groupOfContainer(groups: Group[]): Map<string, string> {
  return new Map(groups.flatMap((g) => g.containers.map((c) => [c, g.id])));
}
