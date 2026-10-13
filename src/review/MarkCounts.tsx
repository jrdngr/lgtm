import { type MarkSetup, markVar } from "./marks";
import type { Progress } from "./state";
import "./MarkCounts.css";

interface Props {
  progress: Progress;
  setup: MarkSetup;
  /** Only colors that don't count as done, for a narrow list. */
  notDone?: boolean;
}

/** A small swatch and count for each mark color the rows have, in key order. */
export function MarkCounts({ progress, setup, notDone }: Props) {
  const counts = setup.slots.filter((s) => {
    const n = progress.marks[s.color] ?? 0;
    return n > 0 && !(notDone && s.done);
  });
  if (counts.length === 0) return null;
  return (
    <span className="mark-counts">
      {counts.map((s) => {
        const n = progress.marks[s.color] ?? 0;
        return (
          <span key={s.color} title={`${n} ${setup.label(s.color)}`}>
            <i style={{ background: markVar(s.color) }} />
            {n}
          </span>
        );
      })}
    </span>
  );
}
