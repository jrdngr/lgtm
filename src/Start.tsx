import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { commands } from "./ipc";

export function Start() {
  const [error, setError] = useState<string>();

  async function pick() {
    const path = await open({ directory: true, title: "Open jj repository" });
    if (!path) return;
    const r = await commands.openRepo(path);
    setError(r.status === "error" ? r.error : undefined);
  }

  return (
    <div className="start">
      <h1>lgtm</h1>
      <button type="button" onClick={pick}>
        Open repository
      </button>
      {error && <p className="error">{error}</p>}
    </div>
  );
}
