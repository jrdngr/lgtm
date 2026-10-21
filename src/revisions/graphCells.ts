/** Where a cell of jj's graph connects: up, down, left, right. */
export interface Cell {
  ch: string;
  u: boolean;
  d: boolean;
  l: boolean;
  r: boolean;
  /** A commit's node, drawn as its glyph. */
  node: boolean;
}

const LINES: Record<string, string> = {
  "│": "ud",
  "|": "ud",
  "─": "lr",
  "-": "lr",
  "╭": "rd",
  "╮": "ld",
  "╰": "ur",
  "╯": "ul",
  "├": "udr",
  "┤": "udl",
  "┼": "udlr",
  "┬": "lrd",
  "┴": "lru",
  "~": "u",
};

/** Characters that draw a diagonal rather than connect to a side. */
const DIAGONALS = new Set(["/", "\\"]);

/** Each graph line's cells and what each connects to. A node connects up or down where the
 * cell above or below connects to it. */
export function graphCells(lines: string[]): Cell[][] {
  const base = lines.map((line) =>
    [...line].map((ch) => {
      const dirs = LINES[ch];
      const node = !dirs && ch.trim() !== "" && !DIAGONALS.has(ch);
      return {
        ch,
        u: !!dirs?.includes("u"),
        d: !!dirs?.includes("d"),
        l: !!dirs?.includes("l"),
        r: !!dirs?.includes("r"),
        node,
      };
    }),
  );
  return base.map((row, i) =>
    row.map((c, j) => (c.node ? { ...c, u: !!base[i - 1]?.[j]?.d, d: !!base[i + 1]?.[j]?.u } : c)),
  );
}

/** Each row's graph cells, found across the whole log so lines join between rows, and the
 * graph's width in cells. */
export function graphOf(rows: { graph: string[] }[]) {
  const lines = rows.flatMap((r) => r.graph.map((g) => g.trimEnd()));
  const cells = graphCells(lines);
  const width = Math.max(1, ...lines.map((l) => [...l].length));
  let at = 0;
  return {
    width,
    cells: rows.map((r) => {
      const mine = cells.slice(at, at + r.graph.length);
      at += r.graph.length;
      return mine;
    }),
  };
}
