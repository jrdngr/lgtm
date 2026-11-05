import type { TurnEdit } from "../bindings";

const W = 276;
const H = 80;
/** The zero line, from the top: added lines get more room than removed ones. */
const MID = H * 0.62;
/** Bars always leave room for at least this many edits, so a few don't fill the chart. */
const MIN_SLOTS = 8;

/** One bar per edit: added lines up in green, removed down in red, the added lines that
 * rewrote earlier ones in amber. The line tracks the running net count. */
export function Churn({ edits }: { edits: TurnEdit[] }) {
  const slots = Math.max(MIN_SLOTS, edits.length);
  const step = W / slots;
  const bw = Math.max(2, step - 6);
  const x = (i: number) => 3 + i * step;
  const scale = Math.max(10, ...edits.flatMap((e) => [e.added, e.removed]));
  let sum = 0;
  const nets = edits.map((e) => {
    sum += e.added - e.removed;
    return sum;
  });
  const netScale = Math.max(10, ...nets.map(Math.abs));
  const ny = (v: number) => MID - (v / netScale) * MID * 0.9;
  const points = [`${x(0)},${ny(0)}`, ...nets.map((v, i) => `${x(i) + bw / 2},${ny(v)}`)];
  const total = edits.reduce(
    (t, e) => ({ a: t.a + e.added, d: t.d + e.removed, r: t.r + e.rewritten }),
    { a: 0, d: 0, r: 0 },
  );
  return (
    <div className="churn">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Churn">
        {Array.from({ length: slots }, (_, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: slots are positions
          <rect key={i} x={x(i)} y={MID} width={bw} height={1} fill="var(--hair)" />
        ))}
        {edits.map((e, i) => {
          const up = (e.added / scale) * MID * 0.95;
          const redo = (Math.min(e.rewritten, e.added) / scale) * MID * 0.95;
          const down = (e.removed / scale) * (H - MID) * 0.95;
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: edits only append
            <g key={i}>
              <rect x={x(i)} y={MID - up} width={bw} height={up} rx={1.5} className="churn-add" />
              {redo > 0 && (
                <rect
                  x={x(i)}
                  y={MID - redo}
                  width={bw}
                  height={redo}
                  rx={1.5}
                  className="churn-redo"
                />
              )}
              <rect x={x(i)} y={MID + 1} width={bw} height={down} rx={1.5} className="churn-del" />
            </g>
          );
        })}
        {edits.length > 0 && (
          <>
            <polyline points={points.join(" ")} className="churn-net" />
            <circle cx={x(edits.length - 1) + bw / 2} cy={ny(sum)} r={2.5} className="churn-dot" />
          </>
        )}
      </svg>
      <div className="churn-total">
        <span className="a">+{total.a}</span>
        <span className="d">−{total.d}</span>
        <span className="r">{total.r} rewritten</span>
      </div>
    </div>
  );
}
