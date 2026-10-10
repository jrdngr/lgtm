import type { FlatFile } from "./model";
import { emptyProgress, type Progress, sumProgress } from "./state";

export interface TreeFile {
  kind: "file";
  name: string;
  file: number;
  progress: Progress;
}

export interface TreeDir {
  kind: "dir";
  /** One or more path segments: single-child directories are merged. */
  name: string;
  path: string;
  children: TreeNode[];
  progress: Progress;
}

export type TreeNode = TreeFile | TreeDir;

function dir(name: string, path: string): TreeDir {
  return { kind: "dir", name, path, children: [], progress: emptyProgress() };
}

/** The review's files as a directory tree with summed progress. Leaves out files `show`
 * rejects, and directories left empty. */
export function buildTree(
  files: FlatFile[],
  progress: Progress[],
  show: (file: number) => boolean,
): TreeDir {
  const root = dir("", "");
  files.forEach((f, file) => {
    if (!show(file)) return;
    const parts = f.dto.path.split("/");
    const name = parts.pop() ?? f.dto.path;
    let at = root;
    for (const part of parts) {
      const path = at.path ? `${at.path}/${part}` : part;
      let next = at.children.find((c): c is TreeDir => c.kind === "dir" && c.path === path);
      if (!next) {
        next = dir(part, path);
        at.children.push(next);
      }
      at = next;
    }
    at.children.push({ kind: "file", name, file, progress: progress[file] });
  });
  return finish(root);
}

function finish(d: TreeDir): TreeDir {
  d.children = d.children.map((c) => (c.kind === "dir" ? compress(finish(c)) : c));
  d.children.sort((a, b) =>
    a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1,
  );
  d.progress = sumProgress(d.children.map((c) => c.progress));
  return d;
}

function compress(d: TreeDir): TreeDir {
  const only = d.children[0];
  if (d.children.length !== 1 || only.kind !== "dir") return d;
  return { ...only, name: `${d.name}/${only.name}` };
}

/** A copy of `s` with `k` added, or removed when it was there. */
export function toggled<T>(s: ReadonlySet<T>, k: T): Set<T> {
  const next = new Set(s);
  if (!next.delete(k)) next.add(k);
  return next;
}
