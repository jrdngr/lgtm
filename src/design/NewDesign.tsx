import { useState } from "react";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { act, selectDesign, setView } from "./store";

/** Names a new design, which makes its folder. */
export function NewDesign({ ws, onDone }: { ws: WorkspaceName; onDone?(): void }) {
  const [name, setName] = useState("");

  async function create() {
    if (!name.trim()) return;
    const r = await act(ws, commands.createDesign(ws, name.trim()));
    if (r.status === "error") return;
    setName("");
    selectDesign(ws, r.data.slug);
    setView(ws, "brief");
    onDone?.();
  }

  return (
    <form
      className="design-new"
      onSubmit={(e) => {
        e.preventDefault();
        create();
      }}
    >
      <input
        value={name}
        placeholder="New design's name"
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onDone?.()}
      />
      <button type="submit" className="primary" disabled={!name.trim()}>
        Create
      </button>
    </form>
  );
}
