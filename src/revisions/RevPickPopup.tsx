import type { ReactNode } from "react";
import type { RevPick } from "./useRevPick";

/** The commits a motion can act on, drawn as the log draws them, in place of the motions. */
export function RevPickPopup({
  pick,
  rowClass,
  rowBody,
  onPick,
}: {
  pick: RevPick;
  /** A log row's classes, as the log gives them. */
  rowClass: (row: number) => string;
  /** A log row's graph and lines. */
  rowBody: (row: number) => ReactNode;
  onPick: (i: number) => void;
}) {
  return (
    <div className="mo-pop mo-revs">
      <div className="mo-head">
        <kbd>⌘J</kbd>
        <span className="mo-typed">{pick.motion.keys}…</span>
        <span className="spacer" />
        <kbd>esc</kbd>
      </div>
      {pick.rows.map((r, i) => (
        // biome-ignore lint/a11y/useKeyWithClickEvents: the log takes the keys
        <div
          key={r.row}
          className={`lr pickable ${rowClass(r.row)}${i === pick.at ? " cursor" : ""}`}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onPick(i)}
        >
          {rowBody(r.row)}
        </div>
      ))}
    </div>
  );
}
