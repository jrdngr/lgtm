import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { commands, type RepoInfo } from "./ipc";

export function Start() {
  const [error, setError] = useState<string>();
  const [gitRoot, setGitRoot] = useState<string | null>(null);
  const [recent, setRecent] = useState<RepoInfo[]>([]);

  useEffect(() => {
    commands.recentRepos().then((r) => r.status === "ok" && setRecent(r.data));
  }, []);

  async function openPath(path: string) {
    const r = await commands.openRepo(path);
    setError(r.status === "error" ? r.error : undefined);
    setGitRoot(r.status === "error" ? await commands.gitRoot(path) : null);
  }

  async function initJj(root: string) {
    const r = await commands.initJj(root);
    setError(r.status === "error" ? r.error : undefined);
    setGitRoot(null);
  }

  async function pick() {
    const path = await open({ directory: true, title: "Open repository" });
    if (path) await openPath(path);
  }

  return (
    <div className="start">
      <h1>lgtm</h1>
      <button type="button" onClick={pick}>
        Open repository
      </button>
      {gitRoot ? (
        <div className="init">
          <p>{gitRoot} is a git repo without jj.</p>
          <button type="button" onClick={() => initJj(gitRoot)}>
            Initialize jj here
          </button>
          <p className="hint">Runs jj git init --colocate. Git keeps working as before.</p>
        </div>
      ) : (
        error && <p className="error">{error}</p>
      )}
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
