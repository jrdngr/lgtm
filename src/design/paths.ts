/** How many of `all` a design entry covers: the file itself, or every file under a folder. */
export function coveredCount(all: string[], entry: string): number {
  const dir = entry.replace(/\/+$/, "");
  return all.filter((p) => p === dir || p.startsWith(`${dir}/`)).length;
}

/** Past this many, the picker stops listing. */
const SHOWN = 30;

/** The folders (ending in `/`) and files of `all` whose path has `query`, folders first and
 * shallow ones before deep, leaving out what `taken` has. */
export function pathChoices(all: string[], query: string, taken: string[]): string[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const folders = new Set<string>();
  for (const p of all) {
    const parts = p.split("/");
    for (let i = 1; i < parts.length; i++) folders.add(`${parts.slice(0, i).join("/")}/`);
  }
  const depth = (p: string) => p.split("/").length;
  const match = (p: string) => p.toLowerCase().includes(q) && !taken.includes(p);
  const dirs = [...folders].filter(match).sort((a, b) => depth(a) - depth(b) || a.localeCompare(b));
  const files = all.filter(match);
  return [...dirs, ...files].slice(0, SHOWN);
}
