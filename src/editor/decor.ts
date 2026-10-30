import type { FileDto } from "../bindings";

/** How the review changes a file, colored as VS Code colors git status. */
export type Decor = "conflict" | "added" | "renamed" | "modified";

/** A folder takes the first of these among its files. VS Code has no fixed order (a folder
 * takes whichever child decoration it finds first), so this follows how much each needs
 * attention: conflicts, then new files, then edits. */
const RANK: Decor[] = ["conflict", "added", "renamed", "modified"];

const LETTER: Record<Decor, string> = { conflict: "!", added: "A", renamed: "R", modified: "M" };

export const decorLetter = (d: Decor) => LETTER[d];

/** Removed files aren't on disk, so as in VS Code they don't color their folders. */
function fileDecor(f: FileDto): Decor | undefined {
  if (f.conflict) return "conflict";
  if (f.status === "removed") return undefined;
  if (f.status === "added" || f.status === "copied") return "added";
  return f.status;
}

export function decorate(files: FileDto[]) {
  const byFile = new Map<string, Decor>();
  const byDir = new Map<string, Decor>();
  const better = (a: Decor, b: Decor | undefined) => !b || RANK.indexOf(a) < RANK.indexOf(b);
  for (const f of files) {
    const d = fileDecor(f);
    if (!d) continue;
    byFile.set(f.path, d);
    for (let i = f.path.lastIndexOf("/"); i > 0; i = f.path.lastIndexOf("/", i - 1)) {
      const dir = f.path.slice(0, i);
      if (better(d, byDir.get(dir))) byDir.set(dir, d);
    }
  }
  return { byFile, byDir };
}
