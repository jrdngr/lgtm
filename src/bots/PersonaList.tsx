import type { Personas } from "../bindings";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { reveal, run } from "../ui/pathMenu";
import { confirmAction, promptText } from "../ui/prompt";
import { useWorkspace, type WorkspaceName } from "../workspaces/context";

interface Props {
  personas: Personas;
  picked: string[];
  onToggle(name: string): void;
  /** After a persona file is copied or trashed. */
  onChanged(): void;
}

/** Asks for a file name, then copies persona file `path` beside itself under it. */
async function duplicate(ws: WorkspaceName, path: string) {
  const file = path.slice(path.lastIndexOf("/") + 1);
  const dot = file.lastIndexOf(".");
  const suggested = dot > 0 ? `${file.slice(0, dot)}-copy${file.slice(dot)}` : `${file}-copy`;
  const name = await promptText("Duplicate Persona", { value: suggested, action: "Duplicate" });
  if (!name) return false;
  openFile(ws, await unwrap(commands.duplicatePersona(path, name)));
  return true;
}

function personaMenu(ws: WorkspaceName, path: string, onChanged: () => void, copyable = true) {
  const done = (p: Promise<boolean>) => run(p.then((changed) => changed && onChanged()));
  async function remove() {
    const name = path.slice(path.lastIndexOf("/") + 1);
    if (!(await confirmAction(`Move ${name} to the Trash?`, "Move to Trash"))) return false;
    await unwrap(commands.trashPath(ws, path));
    return true;
  }
  return (): CtxEntry[] => [
    { label: "Edit", onSelect: () => openFile(ws, path) },
    copyable && { label: "Duplicate…", onSelect: () => done(duplicate(ws, path)) },
    { label: "Reveal in Finder", onSelect: () => reveal(ws, path) },
    "-",
    { label: "Delete Persona…", onSelect: () => done(remove()) },
  ];
}

/** The personas to pick from, each with its description, and ✎ on hover to edit its file.
 * Files that can't be read show their error. */
export function PersonaList({ personas, picked, onToggle, onChanged }: Props) {
  const workspace = useWorkspace();
  const edit = (path: string) => openFile(workspace, path);

  return (
    <ul className="bots-personas">
      {personas.personas.map((p) => {
        const on = picked.includes(p.name);
        return (
          <li
            key={p.name}
            className={on ? undefined : "off"}
            onContextMenu={contextMenu(personaMenu(workspace, p.path, onChanged))}
          >
            <label>
              <input type="checkbox" checked={on} onChange={() => onToggle(p.name)} />
              <span className="bots-persona-name">
                <span className="bots-persona-text">{p.name}</span>
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
        <li
          key={b.path}
          className="bots-broken"
          onContextMenu={contextMenu(personaMenu(workspace, b.path, onChanged, false))}
        >
          <span className="bots-persona-name">
            <span className="bots-persona-text">{b.path.split("/").pop()}</span>
          </span>
          <IconButton icon="edit" label={`Edit ${b.path}`} onClick={() => edit(b.path)} />
          <span className="bots-persona-desc">Can't read it: {b.error}</span>
        </li>
      ))}
    </ul>
  );
}
