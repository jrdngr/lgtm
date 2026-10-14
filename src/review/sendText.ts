import { type DisplayLine, type FlatFile, SIGN } from "./model";

/** The selected rows as text for an agent's prompt: per file and hunk, the path with the
 * new side's line range (the old side's for pure removals), then the rows as a diff. */
export function selectionText(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): string {
  return picked(lines, start, end)
    .map(([file, rows]) => rowsText(files[file], rows))
    .join("\n\n");
}

/** The selected rows' text under their `path:line` or `path:start-end`, per file. */
export function locatedText(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): string {
  return picked(lines, start, end)
    .map(([file, rows]) => {
      const f = files[file];
      return [location(f, rows), ...rows.map((r) => f.rows[r].text)].join("\n");
    })
    .join("\n\n");
}

/** The file-flat rows on display lines `start..=end`, sorted, per file. */
function picked(lines: DisplayLine[], start: number, end: number): [number, number[]][] {
  const byFile = new Map<number, Set<number>>();
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind !== "rows") continue;
    const rows = byFile.get(line.file) ?? new Set();
    for (const r of line.rows) rows.add(r);
    byFile.set(line.file, rows);
  }
  return [...byFile].map(([file, rows]) => [file, [...rows].sort((a, b) => a - b)]);
}

/** The text of the rows on display lines `start..=end`, one per line, or undefined when
 * there are none. */
export function selectedRowText(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): string | undefined {
  const text: string[] = [];
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind === "rows") for (const r of line.rows) text.push(files[line.file].rows[r].text);
  }
  return text.length ? text.join("\n") : undefined;
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

/** `path:line` or `path:start-end` on the new side, or the old side's with "(removed)"
 * when every row is a removal. */
function location(f: FlatFile, rows: number[]): string {
  const rowsOf = rows.map((r) => f.rows[r]);
  const newLines = rowsOf.flatMap((r) => (r.newLine === null ? [] : [r.newLine]));
  const oldLines = rowsOf.flatMap((r) => (r.oldLine === null ? [] : [r.oldLine]));
  const span = newLines.length ? newLines : oldLines;
  const [lo, hi] = [Math.min(...span), Math.max(...span)];
  const where = `${f.dto.path}:${lo === hi ? lo : `${lo}-${hi}`}`;
  return newLines.length ? where : `${where} (removed)`;
}

function block(f: FlatFile, rows: number[]): string {
  const body = rows.map((r) => `${SIGN[f.rows[r].kind]}${f.rows[r].text}`).join("\n");
  return `${location(f, rows)}\n\`\`\`diff\n${body}\n\`\`\``;
}
