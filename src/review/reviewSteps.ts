import type { DisplayLine, FlatFile } from "./model";

/** A display line's file, side and line from 1: the new side's when it shows one. */
export function sideLine(files: FlatFile[], line: DisplayLine | undefined) {
  if (line?.kind !== "rows") return undefined;
  const rows = files[line.file].rows;
  const row = rows[line.right ?? line.rows[0]] ?? rows[line.left ?? -1];
  if (!row) return undefined;
  if (row.newLine !== null) return { file: line.file, isNew: true, line: row.newLine };
  if (row.oldLine !== null) return { file: line.file, isNew: false, line: row.oldLine };
  return undefined;
}

/** The display line of the next or previous syntax item from the cursor's line, on its side.
 * Items the lines don't show are passed over. */
export function itemTarget(
  files: FlatFile[],
  lines: DisplayLine[],
  cursor: number,
  dir: 1 | -1,
): number | undefined {
  const at = sideLine(files, lines[cursor]);
  if (!at) return undefined;
  const dto = files[at.file].dto;
  const steps = new Map((at.isNew ? dto.newSteps : dto.oldSteps).map((s) => [s.line, s]));
  const shown = new Map<number, number>();
  lines.forEach((l, i) => {
    const s = sideLine(files, l);
    if (s && s.file === at.file && s.isNew === at.isNew && !shown.has(s.line)) shown.set(s.line, i);
  });
  let line = at.line;
  for (let n = 0; n < steps.size; n++) {
    const step = steps.get(line);
    const to = dir === 1 ? step?.next : step?.prev;
    if (to === null || to === undefined) return undefined;
    const i = shown.get(to);
    if (i !== undefined) return i;
    line = to;
  }
  return undefined;
}

/** The comment blocks right under the cursor's line, first one first. */
function blockAfter(lines: DisplayLine[], cursor: number) {
  const ids: number[] = [];
  let i = cursor + 1;
  for (; lines[i]?.kind === "comment" || lines[i]?.kind === "info"; i++) {
    const l = lines[i];
    if (l.kind === "comment") ids.push(l.comment);
  }
  return { ids, end: i };
}

/** The comment the cursor is on: the first one under its line. */
export const commentAt = (lines: DisplayLine[], cursor: number): number | undefined =>
  blockAfter(lines, cursor).ids[0];

/** The next or previous comment block past the ones under the cursor's line. */
export function commentStep(lines: DisplayLine[], cursor: number, dir: 1 | -1) {
  const from = dir === 1 ? blockAfter(lines, cursor).end : cursor;
  for (let i = from; i >= 0 && i < lines.length; i += dir) {
    const l = lines[i];
    if (l.kind === "comment") return l.comment;
  }
  return undefined;
}
