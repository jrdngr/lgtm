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
  favorites: string[];
  onToggle(name: string): void;
  onStar(name: string): void;
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
  return contextMenu((): CtxEntry[] => [
    { label: "Edit", onSelect: () => openFile(ws, path) },
    copyable && { label: "Duplicate…", onSelect: () => done(duplicate(ws, path)) },
    { label: "Reveal in Finder", onSelect: () => reveal(ws, path) },
    "-",
    { label: "Delete Persona…", onSelect: () => done(remove()) },
  ]);
}

/** One line per persona: its checkbox, name and description, a star to make it a favorite,
 * and ⋯ for its file. Files that can't be read show their error. */
export function PersonaList({ personas, picked, favorites, onToggle, onStar, onChanged }: Props) {
  const workspace = useWorkspace();
  return (
    <ul className="bots-personas">
      {personas.personas.map((p) => {
        const on = picked.includes(p.name);
        const fav = favorites.includes(p.name);
        const menu = personaMenu(workspace, p.path, onChanged);
        return (
          <li key={p.name} className={on ? undefined : "off"} onContextMenu={menu}>
            <label title={p.description || undefined}>
              <input type="checkbox" checked={on} onChange={() => onToggle(p.name)} />
              <span className="bots-persona-name">{p.name}</span>
              <span className="bots-persona-desc">{p.description}</span>
            </label>
            <IconButton
              icon="star"
              label={fav ? `Unfavorite ${p.name}` : `Favorite ${p.name}`}
              className={`bots-star${fav ? " on" : ""}`}
              aria-pressed={fav}
              onClick={() => onStar(p.name)}
            />
            <IconButton
              icon="more"
              label={`Actions for ${p.name}`}
              className="reveal"
              onClick={menu}
            />
          </li>
        );
      })}
      {personas.broken.map((b) => {
        const name = b.path.split("/").pop();
        const menu = personaMenu(workspace, b.path, onChanged, false);
        return (
          <li
            key={b.path}
            className="bots-broken"
            title={`Can't read it: ${b.error}`}
            onContextMenu={menu}
          >
            <span className="bots-persona-row">
              <span className="bots-warn">!</span>
              <span className="bots-persona-name">{name}</span>
              <span className="bots-persona-desc">Can't read it: {b.error}</span>
            </span>
            <IconButton
              icon="more"
              label={`Actions for ${name}`}
              className="reveal"
              onClick={menu}
            />
          </li>
        );
      })}
    </ul>
  );
}
