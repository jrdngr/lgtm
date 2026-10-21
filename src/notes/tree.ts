import type { NoteDto, NoteLevel, NotesDto } from "../bindings";

export interface NoteFolder {
  /** `/`-separated, empty for the level's top. */
  path: string;
  name: string;
  folders: NoteFolder[];
  /** The last changed first. */
  notes: NoteDto[];
}

/** The parent of folder `path`, empty at the top. */
export const parentOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf("/"), 0));

/** One level's folders and notes as a tree, folders by name. */
export function levelTree(data: NotesDto, level: NoteLevel): NoteFolder {
  const root: NoteFolder = { path: "", name: "", folders: [], notes: [] };
  const byPath = new Map([["", root]]);
  const paths = data.folders
    .filter((f) => f.level === level)
    .map((f) => f.path)
    .sort((a, b) => a.localeCompare(b));
  for (const path of paths) {
    const folder: NoteFolder = {
      path,
      name: path.slice(path.lastIndexOf("/") + 1),
      folders: [],
      notes: [],
    };
    byPath.set(path, folder);
    byPath.get(parentOf(path))?.folders.push(folder);
  }
  for (const n of data.notes) {
    if (n.level === level) byPath.get(n.folder)?.notes.push(n);
  }
  return root;
}
