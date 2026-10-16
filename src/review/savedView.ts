import { type DisplayLine, type FlatFile, hunkKey } from "./model";

/** A review's view as its panel's params keep it through a restart. Files, hunks and lines are
 * keyed by path and line number, so they find their place in a changed diff or drop out. */
export interface SavedView {
  /** The revset the files, hunks, cursor and top line belong to. */
  revset: string;
  /** Files opened (true) or closed by hand, by path. */
  files: [string, boolean][];
  /** Collapsed hunks, by path and new-side start line. */
  hunks: [string, number][];
  /** Comment blocks folded (true) or unfolded by hand, with the `foldState` they were in. */
  comments: [number, boolean, string][];
  /** `lineRef` of the cursor's line. */
  cursor?: string;
  /** `lineRef` of the line at the top of the scrolled diff. */
  top?: string;
}

const isArray = (v: unknown): v is unknown[] => Array.isArray(v);

/** The saved view in a panel's params, if there is one. */
export function parseView(raw: unknown): SavedView | undefined {
  if (typeof raw !== "object" || raw === null) return undefined;
  const v = raw as Partial<SavedView>;
  if (typeof v.revset !== "string") return undefined;
  return {
    revset: v.revset,
    files: isArray(v.files) ? v.files : [],
    hunks: isArray(v.hunks) ? v.hunks : [],
    comments: isArray(v.comments) ? v.comments : [],
    cursor: typeof v.cursor === "string" ? v.cursor : undefined,
    top: typeof v.top === "string" ? v.top : undefined,
  };
}

export function savedHunks(files: FlatFile[], collapsed: ReadonlySet<string>): [string, number][] {
  const out: [string, number][] = [];
  for (const { dto } of files)
    dto.hunks.forEach((h, i) => {
      if (collapsed.has(hunkKey(dto.path, i))) out.push([dto.path, h.newStart]);
    });
  return out;
}

/** The `hunkKey`s of saved hunks that are still in `files`. */
export function restoredHunks(files: FlatFile[], saved: [string, number][]): Set<string> {
  const out = new Set<string>();
  for (const [path, start] of saved) {
    const hunk =
      files.find((f) => f.dto.path === path)?.dto.hunks.findIndex((h) => h.newStart === start) ??
      -1;
    if (hunk >= 0) out.add(hunkKey(path, hunk));
  }
  return out;
}

/** A line's identity apart from its index: a comment or note by id, a row by its line number,
 * a hunk header by where it starts, and anything else by its file. */
export function lineRef(files: FlatFile[], line: DisplayLine | undefined): string | undefined {
  if (!line) return undefined;
  if (line.kind === "comment") return `c${line.comment}`;
  if (line.kind === "note") return `n${line.note}`;
  const f = files[line.file];
  if (!f) return undefined;
  const path = f.dto.path;
  if (line.kind === "hunk") return `h${f.dto.hunks[line.hunk]?.newStart}:${path}`;
  if (line.kind === "rows") {
    const row = f.rows[line.rows[0]];
    return row.newLine === null ? `-${row.oldLine}:${path}` : `+${row.newLine}:${path}`;
  }
  if (line.kind === "context") return `+${line.line.line}:${path}`;
  return `f:${path}`;
}

/** The index of the line `ref` names, else its file's header, else -1. */
export function findLine(files: FlatFile[], lines: DisplayLine[], ref: string | undefined) {
  if (!ref) return -1;
  const at = lines.findIndex((l) => lineRef(files, l) === ref);
  if (at >= 0 || !ref.includes(":")) return at;
  const header = `f:${ref.slice(ref.indexOf(":") + 1)}`;
  return lines.findIndex((l) => l.kind === "file" && lineRef(files, l) === header);
}
