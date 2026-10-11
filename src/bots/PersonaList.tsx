import { useState } from "react";
import type { Personas, Scope } from "../bindings";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { useWorkspace } from "../workspaces/context";

interface Props {
  personas: Personas;
  picked: string[];
  onToggle(name: string): void;
  /** Persona files were added. */
  onChanged(): void;
}

/** The personas to pick from, each with where its file lives and a button to edit it. */
export function PersonaList({ personas, picked, onToggle, onChanged }: Props) {
  const workspace = useWorkspace();
  const [adding, setAdding] = useState<{ name: string; scope: Scope }>();
  const [error, setError] = useState<string>();

  const edit = (path: string) => openFile(workspace, path);

  async function add() {
    if (!adding) return;
    const r = await commands.newPersona(workspace, adding.name.trim(), adding.scope);
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    setAdding(undefined);
    onChanged();
    openFile(workspace, r.data);
  }

  return (
    <>
      <div className="bots-section">
        Personas
        <span className="spacer" />
        <button type="button" onClick={() => setAdding({ name: "", scope: "global" })}>
          New
        </button>
      </div>
      {adding && (
        <form
          className="bots-new"
          onSubmit={(e) => {
            e.preventDefault();
            add();
          }}
        >
          <input
            // biome-ignore lint/a11y/noAutofocus: the form opens to type a name
            autoFocus
            placeholder="docs-bot"
            value={adding.name}
            onChange={(e) => setAdding({ ...adding, name: e.target.value })}
            onKeyDown={(e) => e.key === "Escape" && setAdding(undefined)}
          />
          <select
            value={adding.scope}
            onChange={(e) => setAdding({ ...adding, scope: e.target.value as Scope })}
          >
            <option value="global">Global</option>
            <option value="repo">Repo</option>
          </select>
          <button type="submit" disabled={!adding.name.trim()}>
            Create
          </button>
        </form>
      )}
      <ul className="bots-personas">
        {personas.personas.map((p) => (
          <li key={p.name}>
            <label title={p.description}>
              <input
                type="checkbox"
                checked={picked.includes(p.name)}
                onChange={() => onToggle(p.name)}
              />
              {p.name}
            </label>
            <span className="bots-scope-tag">{p.scope}</span>
            <button type="button" onClick={() => edit(p.path)} title={p.path}>
              Edit
            </button>
          </li>
        ))}
        {personas.broken.map((b) => (
          <li key={b.path} className="bots-broken" title={b.error}>
            <span>{b.path.split("/").pop()}</span>
            <span className="bots-scope-tag">can't read: {b.error}</span>
            <button type="button" onClick={() => edit(b.path)}>
              Edit
            </button>
          </li>
        ))}
      </ul>
      {error && <div className="bots-error">{error}</div>}
    </>
  );
}
