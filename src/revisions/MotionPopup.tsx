import type { Motion } from "./motions";

/** The motions left after `typed`, over the bottom of the log. */
export function MotionPopup({ groups, typed }: { groups: Motion[][]; typed: string }) {
  return (
    <div className="mo-pop">
      <div className="mo-head">
        <kbd>⌘J</kbd>
        {typed && <span className="mo-typed">{typed}…</span>}
        <span className="spacer" />
        <kbd>esc</kbd>
      </div>
      {groups.map((g) => (
        <div key={g[0].keys} className="mo-group">
          {g.map((m) => (
            <div key={m.keys} className="mo">
              <kbd>
                <span className="mo-lit">{m.keys.slice(0, typed.length)}</span>
                {m.keys.slice(typed.length)}
              </kbd>
              <span>{m.label}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
