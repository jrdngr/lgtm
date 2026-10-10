import type { CommentDto } from "../bindings";
import type { Moves } from "./follow";
import { changedRows, type DisplayLine, type FlatFile, fileSlot, markKey } from "./model";
import { rowsText } from "./sendText";

/** A comment being written on a selection, before it's saved. */
export interface Draft {
  file: number;
  /** The selection's changed rows in `file`, in order. */
  rows: number[];
  /** The selection's rows in `file` as a diff. */
  snippet: string;
}

/** A draft on display lines `start..=end`: their changed rows in the last file that has any.
 * Undefined when there are none. */
export function draftFor(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): Draft | undefined {
  const changed = changedRows(files, lines, start, end);
  const file = changed.at(-1)?.file;
  if (file === undefined) return undefined;
  const shown = new Set<number>();
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind === "rows" && line.file === file) for (const r of line.rows) shown.add(r);
  }
  return {
    file,
    rows: changed
      .filter((a) => a.file === file)
      .map((a) => a.row)
      .sort((a, b) => a - b),
    snippet: rowsText(
      files[file],
      [...shown].sort((a, b) => a - b),
    ),
  };
}

/** A draft moved along with its rows, or undefined when they're gone. Without `moves` the
 * review was replaced, so nothing carries over. */
export function followDraft(d: Draft, moves: Moves | undefined): Draft | undefined {
  if (!moves) return undefined;
  const file = moves.files.get(d.file);
  const rows = d.rows.flatMap((r) => {
    const to = moves.rows.get(markKey(d.file, r));
    return to === undefined ? [] : [Number(to.split(":")[1])];
  });
  return file === undefined || rows.length === 0 ? undefined : { ...d, file, rows };
}

/** Comment ids by the `markKey` of the row their block follows, or by `fileSlot` for ones
 * whose code is gone. */
export function commentSlots(comments: CommentDto[], draft: Draft | undefined) {
  const out = new Map<string, number[]>();
  const add = (key: string, id: number) => out.set(key, [...(out.get(key) ?? []), id]);
  for (const c of comments) add(c.rows ? markKey(c.file, c.rows.end) : fileSlot(c.file), c.id);
  if (draft) add(markKey(draft.file, draft.rows[draft.rows.length - 1]), 0);
  return out;
}

/** `markKey`s of every row inside a comment. */
export function commentedRows(comments: CommentDto[]): Set<string> {
  const out = new Set<string>();
  for (const c of comments) {
    if (!c.rows) continue;
    for (let r = c.rows.start; r <= c.rows.end; r++) out.add(markKey(c.file, r));
  }
  return out;
}

/** The comment's code, as it is now or as it was if it's gone, then the comment. */
export function commentText(files: FlatFile[], c: CommentDto): string {
  const f = files[c.file];
  const span = c.rows;
  const code =
    f && span
      ? rowsText(
          f,
          Array.from({ length: span.end - span.start + 1 }, (_, i) => span.start + i),
        )
      : `${c.path} (this code has changed since)\n${c.snippet}`;
  return `${code}\n\n${c.body.trim()}`;
}

/** Parts of an agent reply: plain text, and suggested comments from ```comment blocks. */
export type ReplyPart = { kind: "text" | "suggestion"; text: string };

export function replyParts(reply: string): ReplyPart[] {
  const parts: ReplyPart[] = [];
  const fence = /^```comment[^\n]*\n([\s\S]*?)^```[ \t]*$/gm;
  let at = 0;
  for (const m of reply.matchAll(fence)) {
    const before = reply.slice(at, m.index).trim();
    if (before) parts.push({ kind: "text", text: before });
    parts.push({ kind: "suggestion", text: m[1].replace(/\n$/, "") });
    at = m.index + m[0].length;
  }
  const rest = reply.slice(at).trim();
  if (rest) parts.push({ kind: "text", text: rest });
  return parts;
}
