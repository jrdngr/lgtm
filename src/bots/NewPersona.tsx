import { useState } from "react";
import type { Scope } from "../bindings";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { useWorkspace } from "../workspaces/context";

/** The name and scope of a new persona. Creating it opens its starter file to edit. */
export function NewPersona({ onClose, onCreated }: { onClose(): void; onCreated(): void }) {
  const workspace = useWorkspace();
  const [name, setName] = useState("");
  const [scope, setScope] = useState<Scope>("global");
  const [error, setError] = useState<string>();

  async function add() {
    const r = await commands.newPersona(workspace, name.trim(), scope);
    if (r.status === "error") return setError(r.error);
    onCreated();
    openFile(workspace, r.data);
  }

  return (
    <>
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
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onClose()}
        />
        <select value={scope} onChange={(e) => setScope(e.target.value as Scope)}>
          <option value="global">Global</option>
          <option value="repo">Repo</option>
        </select>
        <button type="submit" disabled={!name.trim()}>
          Create
        </button>
      </form>
      {error && <div className="bots-error">{error}</div>}
    </>
  );
}
