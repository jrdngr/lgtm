import type { Cell } from "./graphCells";

/** A graph cell's width and a line's height, in px. */
export const CELL_W = 8;
const LINE_H = 22;

function Segments({ c, x, y }: { c: Cell; x: number; y: number }) {
  const cx = x + CELL_W / 2;
  const cy = y + LINE_H / 2;
  const right = x + CELL_W;
  const bottom = y + LINE_H;
  if (c.ch === "╭") return <path d={`M${right} ${cy}Q${cx} ${cy} ${cx} ${bottom}`} />;
  if (c.ch === "╮") return <path d={`M${x} ${cy}Q${cx} ${cy} ${cx} ${bottom}`} />;
  if (c.ch === "╰") return <path d={`M${cx} ${y}Q${cx} ${cy} ${right} ${cy}`} />;
  if (c.ch === "╯") return <path d={`M${cx} ${y}Q${cx} ${cy} ${x} ${cy}`} />;
  if (c.ch === "/") return <path d={`M${right} ${y}L${x} ${bottom}`} />;
  if (c.ch === "\\") return <path d={`M${x} ${y}L${right} ${bottom}`} />;
  const d = [
    c.u && `M${cx} ${y}V${cy}`,
    c.d && `M${cx} ${cy}V${bottom}`,
    c.l && `M${x} ${cy}H${cx}`,
    c.r && `M${cx} ${cy}H${right}`,
  ]
    .filter(Boolean)
    .join("");
  if (!d) return null;
  return <path d={d} strokeDasharray={c.ch === "~" ? "2 3" : undefined} />;
}

function Node({ ch, x, y }: { ch: string; x: number; y: number }) {
  const cx = x + CELL_W / 2;
  const cy = y + LINE_H / 2;
  if (ch === "@")
    return (
      <text x={cx} y={cy} className="lg-at" textAnchor="middle" dominantBaseline="central">
        @
      </text>
    );
  if (ch === "◆" || ch === "◇")
    return (
      <path
        className="lg-fixed"
        d={`M${cx} ${cy - 4.5}L${cx + 4} ${cy}L${cx} ${cy + 4.5}L${cx - 4} ${cy}Z`}
      />
    );
  if (ch === "×" || ch === "x")
    return (
      <path
        className="lg-conflict"
        d={`M${cx - 3.5} ${cy - 3.5}L${cx + 3.5} ${cy + 3.5}M${cx + 3.5} ${cy - 3.5}L${cx - 3.5} ${cy + 3.5}`}
      />
    );
  if (ch === "○" || ch === "o") return <circle className="lg-node" cx={cx} cy={cy} r={3.6} />;
  return <circle className="lg-hidden" cx={cx} cy={cy} r={3.6} />;
}

/** jj's graph beside a row's lines, drawn as lines and nodes. */
export function LogGraph({ cells, width }: { cells: Cell[][]; width: number }) {
  return (
    <svg
      className="lg-graph"
      width={width * CELL_W}
      height={cells.length * LINE_H}
      aria-hidden="true"
    >
      {cells.map((line, i) =>
        line.map((c, j) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: cells have no identity
          <g key={`${i}.${j}`}>
            <Segments c={c} x={j * CELL_W} y={i * LINE_H} />
            {c.node && <Node ch={c.ch} x={j * CELL_W} y={i * LINE_H} />}
          </g>
        )),
      )}
    </svg>
  );
}
