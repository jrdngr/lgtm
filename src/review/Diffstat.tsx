import type { LineStat } from "./lineStat";

/** `+added −removed`, in diff colors, leaving out a side with no lines. */
export function Diffstat({ stat }: { stat: LineStat }) {
  if (!stat.added && !stat.removed) return null;
  return (
    <span className="diffstat">
      {stat.added > 0 && <span className="diffstat-add">+{stat.added}</span>}
      {stat.removed > 0 && <span className="diffstat-del">−{stat.removed}</span>}
    </span>
  );
}
