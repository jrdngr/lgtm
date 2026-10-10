import { useEffect, useState } from "react";
import type { DefaultPersona, Installed, Scope } from "../bindings";
import { commands } from "../ipc";
import { useWorkspace } from "../workspaces/context";

const STATE: Record<Installed, string> = {
  no: "",
  yes: "installed",
  changed: "edited, will replace your edits",
};

/** Installs the default personas globally or into the repo. Also restores edited ones. */
export function Installer({ onDone }: { onDone(): void }) {
  const workspace = useWorkspace();
  const [defaults, setDefaults] = useState<DefaultPersona[]>([]);
  const [scope, setScope] = useState<Scope>("global");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string>();

  useEffect(() => {
    commands.defaultPersonas(workspace).then((r) => {
      if (r.status === "ok") setDefaults(r.data);
      else setError(r.error);
    });
  }, [workspace]);

  // Edited files start unchecked, so restoring one is deliberate.
  useEffect(() => {
    setPicked(new Set(defaults.filter((d) => d[scope] !== "changed").map((d) => d.name)));
  }, [defaults, scope]);

  function toggle(name: string) {
    const next = new Set(picked);
    if (!next.delete(name)) next.add(name);
    setPicked(next);
  }

  async function install() {
    const r = await commands.installPersonas(workspace, [...picked], scope);
    if (r.status === "error") return setError(r.error);
    onDone();
  }

  return (
    <div className="bots-installer">
      <div className="bots-section">Install default personas</div>
      <div className="bots-scope">
        {(["global", "repo"] as const).map((s) => (
          <label key={s}>
            <input type="radio" checked={scope === s} onChange={() => setScope(s)} />
            {s === "global" ? "Global" : "This repo"}
          </label>
        ))}
      </div>
      <div className="bots-hint">
        {scope === "global"
          ? "For every repo, in ~/.config/lgtm/personas."
          : "In .lgtm/personas, which adds them to the working copy (@)."}
      </div>
      <div className="bots-list">
        {defaults.map((d) => (
          <label key={d.name} title={d.description}>
            <input type="checkbox" checked={picked.has(d.name)} onChange={() => toggle(d.name)} />
            {d.name}
            {d[scope] !== "no" && (
              <span className={`bots-installed bots-installed-${d[scope]}`}>{STATE[d[scope]]}</span>
            )}
          </label>
        ))}
      </div>
      {error && <div className="bots-error">{error}</div>}
      <div className="bots-actions">
        <button type="button" disabled={picked.size === 0} onClick={install}>
          Install {picked.size}
        </button>
        <button type="button" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}
