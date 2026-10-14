export interface DiffLine {
  kind: "ctx" | "add" | "del";
  /** 1-based, in the new text for context and additions, in the old one for removals. */
  line: number;
  text: string;
}

/** Lines kept around each change. */
const CONTEXT = 2;

/** `before` to `after` by lines, as hunks of changed lines with a little context. A removal
 * comes before what replaced it. */
export function lineDiff(before: string, after: string): DiffLine[][] {
  const a = before === "" ? [] : before.replace(/\n$/, "").split("\n");
  const b = after === "" ? [] : after.replace(/\n$/, "").split("\n");
  // Lines the two share at either end take no table.
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (
    tail < a.length - head &&
    tail < b.length - head &&
    a[a.length - 1 - tail] === b[b.length - 1 - tail]
  )
    tail++;
  const ma = a.slice(head, a.length - tail);
  const mb = b.slice(head, b.length - tail);
  const lcs = Array.from({ length: ma.length + 1 }, () => new Array<number>(mb.length + 1).fill(0));
  for (let i = ma.length - 1; i >= 0; i--) {
    for (let j = mb.length - 1; j >= 0; j--) {
      lcs[i][j] = ma[i] === mb[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const all: DiffLine[] = a.slice(0, head).map((text, i) => ({ kind: "ctx", line: i + 1, text }));
  let i = 0;
  let j = 0;
  while (i < ma.length || j < mb.length) {
    if (i < ma.length && j < mb.length && ma[i] === mb[j]) {
      all.push({ kind: "ctx", line: head + j + 1, text: mb[j] });
      i++;
      j++;
    } else if (i < ma.length && (j === mb.length || lcs[i + 1][j] >= lcs[i][j + 1])) {
      all.push({ kind: "del", line: head + i + 1, text: ma[i++] });
    } else {
      all.push({ kind: "add", line: head + j + 1, text: mb[j++] });
    }
  }
  const at = b.length - tail;
  for (const [k, text] of b.slice(at).entries()) all.push({ kind: "ctx", line: at + k + 1, text });

  // Changed lines with CONTEXT lines either side, overlapping runs merged.
  const keep = all.map(() => false);
  for (const [k, l] of all.entries()) {
    if (l.kind === "ctx") continue;
    for (let n = Math.max(0, k - CONTEXT); n <= Math.min(all.length - 1, k + CONTEXT); n++)
      keep[n] = true;
  }
  const hunks: DiffLine[][] = [];
  let run: DiffLine[] | undefined;
  for (const [k, l] of all.entries()) {
    if (!keep[k]) {
      run = undefined;
      continue;
    }
    if (!run) {
      run = [];
      hunks.push(run);
    }
    run.push(l);
  }
  return hunks;
}

/** How many lines `hunks` add and remove. */
export function lineCounts(hunks: DiffLine[][]) {
  const lines = hunks.flat();
  return {
    added: lines.filter((l) => l.kind === "add").length,
    removed: lines.filter((l) => l.kind === "del").length,
  };
}
