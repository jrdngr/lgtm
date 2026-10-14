import { useEffect, useState } from "react";
import type { DefaultPersona, Installed } from "../bindings";
import { commands } from "../ipc";

const STATE: Record<Installed, string> = {
  no: "",
  yes: "installed",
  outdated: "older version, will update",
  changed: "edited, will replace your edits",
};

/** Installs the default personas into ~/.config/lgtm/personas. Also updates older versions and
 * restores edited ones. */
export function Installer({ onDone }: { onDone(): void }) {
  const [defaults, setDefaults] = useState<DefaultPersona[]>([]);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string>();

  useEffect(() => {
    commands.defaultPersonas().then(setDefaults);
  }, []);

  // Edited files start unchecked, so restoring one is deliberate.
  useEffect(() => {
    setPicked(new Set(defaults.filter((d) => d.installed !== "changed").map((d) => d.name)));
  }, [defaults]);

  function toggle(name: string) {
    const next = new Set(picked);
    if (!next.delete(name)) next.add(name);
    setPicked(next);
  }

  async function install() {
    const r = await commands.installPersonas([...picked]);
    if (r.status === "error") return setError(r.error);
    onDone();
  }

  return (
    <div className="bots-installer">
      <div className="bots-section">
        <span className="bots-label">Install default personas</span>
        <span className="spacer" />
        <span className="bots-links">
          <button type="button" onClick={() => setPicked(new Set(defaults.map((d) => d.name)))}>
            All
          </button>
          <i>·</i>
          <button type="button" onClick={() => setPicked(new Set())}>
            None
          </button>
        </span>
      </div>
      <div className="bots-hint">For every repo, in ~/.config/lgtm/personas.</div>
      <div className="bots-list">
        {defaults.map((d) => (
          <label key={d.name} title={d.description}>
            <input type="checkbox" checked={picked.has(d.name)} onChange={() => toggle(d.name)} />
            {d.name}
            {d.installed !== "no" && (
              <span className={`bots-installed bots-installed-${d.installed}`}>
                {STATE[d.installed]}
              </span>
            )}
          </label>
        ))}
      </div>
      {error && <div className="bots-error">{error}</div>}
      <div className="bots-actions">
        <button type="button" className="primary" disabled={picked.size === 0} onClick={install}>
          Install {picked.size}
        </button>
        <button type="button" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}
