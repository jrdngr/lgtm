import type { DisplayLine, FlatFile } from "../review/model";

/** The working-copy file and new-side line a review line shows, for opening it in the
 * editor. A removed row takes the new line before it. Undefined for a removed file. */
export function editorAt(
  files: FlatFile[],
  line: DisplayLine | undefined,
): { path: string; line?: number } | undefined {
  if (!line) return undefined;
  const f = files[line.file];
  if (!f || f.dto.status === "removed") return undefined;
  const row =
    line.kind === "rows" ? (line.right ?? line.rows[0]) : line.kind === "gap" ? line.rows[0] : -1;
  if (row < 0) return { path: f.dto.path };
  for (let r = row; r >= 0; r--) {
    const at = f.rows[r].newLine;
    if (at !== null) return { path: f.dto.path, line: at };
  }
  return { path: f.dto.path, line: 1 };
}
