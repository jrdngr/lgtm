import type { Personas } from "../bindings";
import { openFile } from "../editor/open";
import { IconButton } from "../ui/icons";
import { useWorkspace } from "../workspaces/context";

interface Props {
  personas: Personas;
  picked: string[];
  onToggle(name: string): void;
}

/** The personas to pick from, each with its description, and ✎ on hover to edit its file.
 * Files that can't be read show their error. */
export function PersonaList({ personas, picked, onToggle }: Props) {
  const workspace = useWorkspace();
  const edit = (path: string) => openFile(workspace, path);

  return (
    <ul className="bots-personas">
      {personas.personas.map((p) => {
        const on = picked.includes(p.name);
        return (
          <li key={p.name} className={on ? undefined : "off"}>
            <label>
              <input type="checkbox" checked={on} onChange={() => onToggle(p.name)} />
              <span className="bots-persona-name">
                {p.name}
                {p.scope === "repo" && <span className="pill">repo</span>}
              </span>
            </label>
            <IconButton
              icon="edit"
              label={`Edit ${p.path}`}
              className="reveal"
              onClick={() => edit(p.path)}
            />
            {p.description && <span className="bots-persona-desc">{p.description}</span>}
          </li>
        );
      })}
      {personas.broken.map((b) => (
        <li key={b.path} className="bots-broken">
          <span className="bots-persona-name">{b.path.split("/").pop()}</span>
          <IconButton icon="edit" label={`Edit ${b.path}`} onClick={() => edit(b.path)} />
          <span className="bots-persona-desc">Can't read it: {b.error}</span>
        </li>
      ))}
    </ul>
  );
}
