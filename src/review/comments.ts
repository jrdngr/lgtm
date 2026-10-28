import type { CommentDto, Message } from "../bindings";
import type { Moves } from "./follow";
import {
  changedRows,
  type DisplayLine,
  type FlatFile,
  fileSlot,
  markKey,
  NEW_COMMENT,
} from "./model";
import { rowsRefs } from "./sendText";
import { unified } from "./snippet";
import { UNTRUSTED_NOTE, untrusted } from "./untrusted";

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
    snippet: unified([...shown].sort((a, b) => a - b).map((r) => files[file].rows[r])),
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
  if (draft) add(markKey(draft.file, draft.rows[draft.rows.length - 1]), NEW_COMMENT);
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

/** Where the comment's code is, or the code as it was if it's gone, then the comment and its
 * conversation. A comment on the PR comes with its posts
 * instead, and a quote with what it quotes, fenced as GitHub's. */
export function commentText(files: FlatFile[], c: CommentDto): string {
  const quote = c.thread?.quote;
  if (quote) {
    return [UNTRUSTED_NOTE, untrusted(quote.text, quote.author), c.body.trim()]
      .filter(Boolean)
      .join("\n\n");
  }
  if (!c.path) {
    const posts = c.github?.posts.map((p) => untrusted(p.body.trim(), p.author)) ?? [];
    return [posts.length ? UNTRUSTED_NOTE : "", ...posts, c.body.trim()]
      .filter(Boolean)
      .join("\n\n");
  }
  return [codeText(files, c), c.body.trim(), conversationText(c.messages)]
    .filter(Boolean)
    .join("\n\n");
}

/** Where a comment's code is, or the code as it was if it's gone. */
export function codeText(files: FlatFile[], c: CommentDto): string {
  const f = files[c.file];
  const span = c.rows;
  return f && span
    ? rowsRefs(
        f,
        Array.from({ length: span.end - span.start + 1 }, (_, i) => span.start + i),
      )
    : `${c.path} (this code has changed since)\n${c.snippet}`;
}

/** A comment's conversation, one message after another. Empty without one. */
export function conversationText(messages: Message[]): string {
  const said = messages.flatMap((m) => {
    const text = m.text.trim();
    return text ? [`${m.author === "agent" ? "Agent" : "Me"}:\n${text}`] : [];
  });
  return said.length ? ["Our conversation about it:", ...said].join("\n\n") : "";
}

/** Where a row is, as `file.ts:12`, or just the file name without a row. */
export function rowLabel(files: FlatFile[], file: number, path: string, row?: number): string {
  const r = row !== undefined ? files[file]?.rows[row] : undefined;
  const line = r ? (r.newLine ?? r.oldLine) : undefined;
  return `${path.split("/").pop()}${line != null ? `:${line}` : ""}`;
}
