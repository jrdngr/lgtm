import type { RowDto } from "../bindings";

/** A run of changed lines, by new-side line: added lines, changed lines (added in place of
 * removed ones), or removed lines, which sit after line `from - 1`. */
export interface DiffBlock {
  kind: "added" | "changed" | "removed";
  /** The first new-side line, 1-based. For removed lines, the line after them. */
  from: number;
  /** The last new-side line; `from - 1` for removed lines. */
  to: number;
  /** The old side's lines the block takes the place of. */
  removed: string[];
}

/** The review rows' changed runs, in order. */
export function diffBlocks(rows: RowDto[]): DiffBlock[] {
  const out: DiffBlock[] = [];
  /** The new-side line the next added row would take. */
  let next = 1;
  let removed: string[] = [];
  let added = 0;
  const flush = () => {
    if (!removed.length && !added) return;
    const kind = !added ? "removed" : removed.length ? "changed" : "added";
    out.push({ kind, from: next - added, to: next - 1, removed });
    removed = [];
    added = 0;
  };
  for (const r of rows) {
    if (r.kind === "removed") {
      // Removed rows after added ones start a run of their own.
      if (added) flush();
      removed.push(r.text);
      continue;
    }
    if (r.kind === "added") {
      next = (r.newLine ?? next) + 1;
      added++;
      continue;
    }
    flush();
    next = (r.newLine ?? next) + 1;
  }
  flush();
  return out;
}

/** Where each of `from`'s lines is in `to`, 1-based, or undefined for lines `to` changed. Lines
 * past a common start and end are matched by longest common subsequence while that's cheap. */
export function lineMap(from: string[], to: string[]): (number | undefined)[] {
  const map: (number | undefined)[] = new Array(from.length + 1).fill(undefined);
  let start = 0;
  while (start < from.length && start < to.length && from[start] === to[start]) {
    map[start + 1] = start + 1;
    start++;
  }
  let end = 0;
  while (
    end < from.length - start &&
    end < to.length - start &&
    from[from.length - 1 - end] === to[to.length - 1 - end]
  ) {
    map[from.length - end] = to.length - end;
    end++;
  }
  const a = from.slice(start, from.length - end);
  const b = to.slice(start, to.length - end);
  if (!a.length || !b.length || a.length * b.length > 4_000_000) return map;
  // lcs[i][j]: the longest common run of a[i..] and b[j..].
  const lcs = Array.from({ length: a.length + 1 }, () => new Uint32Array(b.length + 1));
  for (let i = a.length - 1; i >= 0; i--)
    for (let j = b.length - 1; j >= 0; j--)
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
  for (let i = 0, j = 0; i < a.length && j < b.length; ) {
    if (a[i] === b[j]) {
      map[start + i + 1] = start + j + 1;
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) i++;
    else j++;
  }
  return map;
}

/** The blocks moved onto the editor's lines through `map`. A block whose lines all moved
 * keeps its shape and its `removed` array, which ties it to its source; one whose lines were
 * edited since is dropped. */
export function mapBlocks(blocks: DiffBlock[], map: (number | undefined)[], lines: number) {
  const out: DiffBlock[] = [];
  for (const b of blocks) {
    if (b.kind === "removed") {
      // Anchored on the line before, or the top of the file.
      const before = b.from - 1;
      const at = before === 0 ? 0 : map[before];
      if (at !== undefined && at <= lines) out.push({ ...b, from: at + 1, to: at });
      continue;
    }
    const from = map[b.from];
    const to = map[b.to];
    if (from === undefined || to === undefined || to - from !== b.to - b.from) continue;
    out.push({ ...b, from, to });
  }
  return out;
}
