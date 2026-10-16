/** `path` relative to the repo at `root` when it's inside it, else as it is. Folders keep
 * their trailing `/`. */
export function repoRelative(path: string, root: string | undefined): string {
  if (!root) return path;
  const base = root.endsWith("/") ? root : `${root}/`;
  return path.startsWith(base) && path !== base ? path.slice(base.length) : path;
}

/** A Read first path split for its row: the name, the folder it's in, and whether it's a
 * folder itself (it ends in `/`). */
export function splitPath(path: string): { name: string; dir: string; folder: boolean } {
  const folder = path.endsWith("/");
  const trimmed = folder ? path.slice(0, -1) : path;
  const at = trimmed.lastIndexOf("/");
  return at < 0
    ? { name: trimmed, dir: "", folder }
    : { name: trimmed.slice(at + 1), dir: trimmed.slice(0, at) || "/", folder };
}

/** `paths` added to `list`, skipping ones it has. */
export const addPaths = (list: string[], paths: string[]) => [
  ...list,
  ...paths.filter((p, i) => p && !list.includes(p) && paths.indexOf(p) === i),
];
