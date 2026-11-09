/** Code folding for the modal keys: ranges of lines from the syntax tree, or from indent
 * where there's no grammar, and which of them are closed. Mirrors
 * `crates/lgtm/src/edit/fold.rs`. */

import type { Buffer } from "./buffer";
import type { Fold } from "./keymap";

/** A foldable run of lines: `start` stays in view, `start + 1..=end` hide when it's closed. */
export interface Range {
  start: number;
  end: number;
  /** How many ranges hold it. */
  depth: number;
}

/** Every foldable range from spans by start line, sorted, with its depth. */
export function ranges(spans: Map<number, number>): Range[] {
  const sorted = [...spans].sort((a, b) => a[0] - b[0]);
  return sorted.map(([start, end]) => ({
    start,
    end,
    depth: sorted.filter(([s, e]) => s < start && end <= e).length,
  }));
}

/** Keeps the longest span for each start line. */
function add(ends: Map<number, number>, start: number, end: number) {
  if (end > start) ends.set(start, Math.max(ends.get(start) ?? end, end));
}

/** For a file with no grammar: a line and the more indented lines after it. */
export function indentSpans(b: Buffer): Map<number, number> {
  const ends = new Map<number, number>();
  const last = b.lastLine();
  const blank = (l: number) => b.lineEnd(l) === b.lineStart(l) + b.indent(l).length;
  const depth = (l: number) => [...b.indent(l)].reduce((n, c) => n + (c === "\t" ? 4 : 1), 0);
  for (let line = 0; line <= last; line++) {
    if (blank(line)) continue;
    const d = depth(line);
    let end = line;
    for (let next = line + 1; next <= last; next++) {
      if (blank(next)) continue;
      if (depth(next) <= d) break;
      end = next;
    }
    add(ends, line, end);
  }
  return ends;
}

export class Folds {
  ranges: Range[] = [];
  /** Closed ranges, by start line. */
  private closed = new Set<number>();
  /** Ranges this deep or deeper are closed, after `zm` or `zr`. */
  private level: number | undefined;

  /** Takes new ranges, keeping closed what still starts on the same lines. */
  update(ranges: Range[]) {
    this.ranges = ranges;
    const starts = new Set(ranges.map((r) => r.start));
    for (const s of this.closed) if (!starts.has(s)) this.closed.delete(s);
  }

  private maxDepth() {
    return Math.max(0, ...this.ranges.map((r) => r.depth + 1));
  }

  isClosed(start: number) {
    return this.closed.has(start);
  }

  /** The innermost range starting on `line`, else the innermost holding it. */
  private at(line: number, pick: (r: Range) => boolean): Range | undefined {
    const deepest = (rs: Range[]) =>
      rs.reduce<Range | undefined>((a, r) => (!a || r.depth > a.depth ? r : a), undefined);
    return (
      deepest(this.ranges.filter((r) => r.start === line && pick(r))) ??
      deepest(this.ranges.filter((r) => r.start <= line && line <= r.end && pick(r)))
    );
  }

  apply(fold: Fold, line: number) {
    switch (fold) {
      case "toggle": {
        const r = this.at(line, (x) => this.isClosed(x.start));
        if (r) this.closed.delete(r.start);
        else this.close(line);
        break;
      }
      case "close":
        this.close(line);
        break;
      case "open": {
        const r = this.ranges
          .filter((x) => x.start <= line && line <= x.end && this.isClosed(x.start))
          .reduce<Range | undefined>((a, x) => (!a || x.depth < a.depth ? x : a), undefined);
        if (r) this.closed.delete(r.start);
        break;
      }
      case "closeAll":
        this.setLevel(0);
        break;
      case "openAll":
        this.setLevel(this.maxDepth());
        break;
      case "closeLevel":
        this.setLevel(Math.max((this.level ?? this.maxDepth()) - 1, 0));
        break;
      case "openLevel":
        this.setLevel(Math.min((this.level ?? this.maxDepth()) + 1, this.maxDepth()));
        break;
    }
  }

  private close(line: number) {
    const r = this.at(line, (x) => !this.isClosed(x.start));
    if (r) this.closed.add(r.start);
  }

  private setLevel(level: number) {
    this.level = level;
    this.closed = new Set(this.ranges.filter((r) => r.depth >= level).map((r) => r.start));
  }

  /** Opens the closed ranges that hide `line`. */
  reveal(line: number) {
    for (const r of this.ranges) {
      if (r.start < line && line <= r.end) this.closed.delete(r.start);
    }
  }

  /** The closed ranges that hide lines, outermost only, by start line. */
  hidden(): [number, number][] {
    const out: [number, number][] = [];
    for (const r of this.ranges) {
      if (!this.isClosed(r.start)) continue;
      const last = out.at(-1);
      if (last && r.start <= last[1]) continue;
      out.push([r.start, r.end]);
    }
    return out;
  }
}
