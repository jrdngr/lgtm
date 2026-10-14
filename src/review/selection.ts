import {
  type DisplayLine,
  enclosingSymbols,
  type FlatFile,
  fileRange,
  hunkRange,
  symbolRange,
  symbolSide,
} from "./model";

export type Level = "line" | "range" | "function" | "hunk" | "file" | "review";

/** A contiguous run of display lines. Actions use the changed rows inside it. */
export interface Selection {
  start: number;
  end: number;
  level: Level;
  /** Status bar text, e.g. "function: get_user". */
  label: string;
}

export function lineSelection(i: number): Selection {
  return { start: i, end: i, level: "line", label: "line" };
}

export function rangeSelection(a: number, b: number): Selection {
  const [start, end] = a <= b ? [a, b] : [b, a];
  if (start === end) return lineSelection(start);
  return { start, end, level: "range", label: `range: ${end - start + 1} lines` };
}

export function reviewSelection(lines: DisplayLine[]): Selection {
  return { start: 0, end: lines.length - 1, level: "review", label: "review" };
}

export function hunkSelection(lines: DisplayLine[], file: number, hunk: number): Selection {
  const [start, end] = hunkRange(lines, file, hunk);
  return { start, end, level: "hunk", label: "hunk" };
}

export function fileSelection(files: FlatFile[], lines: DisplayLine[], file: number): Selection {
  const [start, end] = fileRange(lines, file);
  return { start, end, level: "file", label: `file: ${files[file].dto.path}` };
}

/** Every unit around display line `i`, smallest first: functions (innermost first), hunk,
 * file, review. Functions are left out where the parse has errors. */
export function units(files: FlatFile[], lines: DisplayLine[], i: number): Selection[] {
  const line = lines[i];
  if (!line) return [];
  const out: Selection[] = [];
  if (line.kind === "rows") {
    // In split view the new side decides, since that's the code under review.
    const row = line.right ?? line.rows[line.rows.length - 1];
    const side = symbolSide(files[line.file].rows[row])?.side;
    for (const sym of enclosingSymbols(files[line.file], row)) {
      const range = side && symbolRange(files, lines, line.file, sym, side);
      if (range) {
        out.push({
          start: range[0],
          end: range[1],
          level: "function",
          label: `${sym.kind}: ${sym.name}`,
        });
      }
    }
  }
  if (line.kind !== "file") out.push(hunkSelection(lines, line.file, line.hunk));
  out.push(fileSelection(files, lines, line.file));
  out.push(reviewSelection(lines));
  return out;
}

/** The next unit around the cursor that's strictly bigger than `sel`. */
export function expand(
  files: FlatFile[],
  lines: DisplayLine[],
  cursor: number,
  sel: Selection,
): Selection | undefined {
  return units(files, lines, cursor).find(
    (u) => u.start <= sel.start && u.end >= sel.end && u.end - u.start > sel.end - sel.start,
  );
}
