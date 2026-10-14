import { useState } from "react";
import { filterShortcuts, SHORTCUT_GROUPS } from "./shortcuts";
import "./shortcuts.css";

/** Every keyboard shortcut by where it works, with a filter. */
export function ShortcutsPanel() {
  const [query, setQuery] = useState("");
  const groups = filterShortcuts(SHORTCUT_GROUPS, query);
  return (
    <div className="shortcuts">
      <div className="shortcuts-page">
        <header className="shortcuts-head">
          <h1>Keyboard Shortcuts</h1>
          <input
            type="search"
            placeholder="Filter"
            aria-label="Filter shortcuts"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </header>
        {groups.length === 0 && <div className="placeholder">No shortcut matches.</div>}
        <div className="shortcuts-groups">
          {groups.map((g) => (
            <section key={g.title} className="shortcuts-group" aria-label={g.title}>
              <h2>{g.title}</h2>
              <dl>
                {g.shortcuts.map((s) => (
                  <div key={s.keys.join() + s.action} className="shortcuts-row">
                    <dt>
                      {s.keys.map((k) => (
                        <kbd key={k}>{k}</kbd>
                      ))}
                    </dt>
                    <dd>{s.action}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
