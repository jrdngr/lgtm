import type { BookmarkPick } from "./useBookmarkPick";

/** The bookmarks a motion can move, numbered, in place of the motions. */
export function BookmarkPopup({
  pick,
  onPick,
}: {
  pick: BookmarkPick;
  onPick: (i: number) => void;
}) {
  return (
    <div className="mo-pop">
      <div className="mo-head">
        <kbd>⌘J</kbd>
        <span className="mo-typed">{pick.motion.keys}…</span>
        <span className="spacer" />
        <kbd>esc</kbd>
      </div>
      {pick.spots.map((s, i) => (
        // biome-ignore lint/a11y/useKeyWithClickEvents: the log takes the keys
        <div
          key={s.name}
          className={`mo mo-pick${i === pick.at ? " on" : ""}`}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onPick(i)}
        >
          <kbd>{i < 9 ? i + 1 : ""}</kbd>
          <span className="mo-bookmark">{s.name}</span>
          <span className="lr-id">
            <span className="lr-unique">{s.change.slice(0, s.unique)}</span>
            {s.change.slice(s.unique)}
          </span>
        </div>
      ))}
    </div>
  );
}
