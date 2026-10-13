import type { ThreadDto } from "../bindings";
import { STAGES } from "./stages";

/** How far a PR's threads are: one segment per stage, sized by its count. */
export function StageBar({ threads }: { threads: ThreadDto[] }) {
  if (threads.length === 0) return null;
  const counts = STAGES.map(
    ([s, name, tone]) =>
      [s, name, tone, threads.filter((t) => t.state.stage === s).length] as const,
  ).filter(([, , , n]) => n > 0);
  const open = threads.filter((t) => t.state.stage !== "done").length;
  return (
    <div className="stage-bar-row">
      <div
        className="stage-bar"
        role="img"
        aria-label={counts.map(([, n, , c]) => `${c} ${n}`).join(", ")}
      >
        {counts.map(([s, name, tone, n]) => (
          <span
            key={s}
            className={`stage-seg ${tone || "grey"}`}
            style={{ flexGrow: n }}
            title={`${name}: ${n}`}
          />
        ))}
      </div>
      <span className="stage-open">{open} open</span>
    </div>
  );
}
