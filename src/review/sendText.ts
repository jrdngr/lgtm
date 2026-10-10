import type { DisplayLine, FlatFile } from "./model";

const SIGN = { context: " ", added: "+", removed: "-" } as const;

/** The selected rows as text for an agent's prompt: per file and hunk, the path with the
 * new side's line range (the old side's for pure removals), then the rows as a diff. */
export function selectionText(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): string {
  const picked = new Map<number, Set<number>>();
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind !== "rows") continue;
    const rows = picked.get(line.file) ?? new Set();
    for (const r of line.rows) rows.add(r);
    picked.set(line.file, rows);
  }
  return [...picked]
    .map(([file, rows]) =>
      rowsText(
        files[file],
        [...rows].sort((a, b) => a - b),
      ),
    )
    .join("\n\n");
}

/** File-flat `rows` of one file, in order, as one diff block per hunk. */
export function rowsText(f: FlatFile, rows: number[]): string {
  const blocks: string[] = [];
  // One block per hunk, since rows in different hunks aren't adjacent.
  let group: number[] = [];
  const flush = () => {
    if (group.length) blocks.push(block(f, group));
    group = [];
  };
  for (const r of rows) {
    if (group.length && f.rowHunk[r] !== f.rowHunk[group[0]]) flush();
    group.push(r);
  }
  flush();
  return blocks.join("\n\n");
}

function block(f: FlatFile, rows: number[]): string {
  const rowsOf = rows.map((r) => f.rows[r]);
  const newLines = rowsOf.flatMap((r) => (r.newLine === null ? [] : [r.newLine]));
  const oldLines = rowsOf.flatMap((r) => (r.oldLine === null ? [] : [r.oldLine]));
  const span = newLines.length ? newLines : oldLines;
  const [lo, hi] = [Math.min(...span), Math.max(...span)];
  const where = `${f.dto.path}:${lo === hi ? lo : `${lo}-${hi}`}`;
  const label = newLines.length ? where : `${where} (removed)`;
  const body = rowsOf.map((r) => `${SIGN[r.kind]}${r.text}`).join("\n");
  return `${label}\n\`\`\`diff\n${body}\n\`\`\``;
}
