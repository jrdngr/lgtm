import type { FileDto } from "../bindings";

export interface LineStat {
  added: number;
  removed: number;
}

/** How many lines a file's diff adds and removes. */
export function lineStat(file: FileDto): LineStat {
  const stat = { added: 0, removed: 0 };
  for (const h of file.hunks) {
    for (const r of h.rows) {
      if (r.kind === "added") stat.added++;
      else if (r.kind === "removed") stat.removed++;
    }
  }
  return stat;
}

export const sumStats = (stats: LineStat[]): LineStat =>
  stats.reduce((a, s) => ({ added: a.added + s.added, removed: a.removed + s.removed }), {
    added: 0,
    removed: 0,
  });
