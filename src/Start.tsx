import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { commands, type RepoInfo } from "./ipc";

export function Start() {
  const [error, setError] = useState<string>();
  const [recent, setRecent] = useState<RepoInfo[]>([]);

  useEffect(() => {
    commands.recentRepos().then((r) => r.status === "ok" && setRecent(r.data));
  }, []);

  async function openPath(path: string) {
    const r = await commands.openRepo(path);
    setError(r.status === "error" ? r.error : undefined);
  }

  async function pick() {
    const path = await open({ directory: true, title: "Open jj repository" });
    if (path) await openPath(path);
  }

  return (
    <div className="start">
      <h1>lgtm</h1>
      <button type="button" onClick={pick}>
        Open repository
      </button>
      {error && <p className="error">{error}</p>}
      {recent.length > 0 && (
        <section className="recent">
          <h2>Open Recent</h2>
          <ul>
            {recent.map((r) => (
              <li key={r.root}>
                <button type="button" onClick={() => openPath(r.root)} title={r.root}>
                  <span className="name">{r.name}</span>
                  <span className="root">{r.root}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
