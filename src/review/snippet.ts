import type { RowDto } from "../bindings";
import { SIGN } from "../model/sign";

/** Rows as unified diff hunks: a `@@ -old +new @@` header wherever the line numbers jump,
 * then each row with its sign. Matches the snippets the backend writes. */
export function unified(rows: RowDto[]): string {
  const out: string[] = [];
  let head: { at: number; old: [number, number]; new: [number, number] } | undefined;
  const header = (h: NonNullable<typeof head>) =>
    `@@ -${h.old[0]},${h.old[1]} +${h.new[0]},${h.new[1]} @@`;
  const follows = ([start, count]: [number, number], line: number | null) =>
    count === 0 || line === null || line === start + count;
  for (const r of rows) {
    if (!head || !follows(head.old, r.oldLine) || !follows(head.new, r.newLine)) {
      if (head) out[head.at] = header(head);
      out.push("");
      head = { at: out.length - 1, old: [0, 0], new: [0, 0] };
    }
    for (const [side, line] of [
      [head.old, r.oldLine],
      [head.new, r.newLine],
    ] as const) {
      if (line === null) continue;
      if (side[1] === 0) side[0] = line;
      side[1]++;
    }
    out.push(`${SIGN[r.kind]}${r.text}`);
  }
  if (head) out[head.at] = header(head);
  return out.join("\n");
}

export interface SnippetLine {
  kind: "hunk" | "context" | "added" | "removed";
  text: string;
  /** Line numbers, when the snippet has `@@` headers to count from. */
  old?: number;
  new?: number;
}

const HEADER = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;
const KINDS = { " ": "context", "+": "added", "-": "removed" } as const;

/** A snippet's lines, numbered from its `@@` headers like GitHub's diff hunks. Lines that
 * aren't diff rows, like code fences, are left out. */
export function snippetLines(snippet: string): SnippetLine[] {
  const out: SnippetLine[] = [];
  let at: { old: number; new: number } | undefined;
  for (const line of snippet.split("\n")) {
    const h = HEADER.exec(line);
    if (h) {
      at = { old: Number(h[1]), new: Number(h[2]) };
      out.push({ kind: "hunk", text: line });
      continue;
    }
    const kind = KINDS[line[0] as keyof typeof KINDS];
    if (!kind) continue;
    const row: SnippetLine = { kind, text: line.slice(1) };
    if (at) {
      if (kind !== "added") row.old = at.old++;
      if (kind !== "removed") row.new = at.new++;
    }
    out.push(row);
  }
  return out;
}
