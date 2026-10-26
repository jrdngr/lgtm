import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import type { ProjectSummary } from "./bindings";
import { commands, type RepoInfo } from "./ipc";
import { contextMenu } from "./ui/contextMenu";

const closedProjects = (set: (p: ProjectSummary[]) => void) =>
  commands.closedProjects().then((r) => r.status === "ok" && set(r.data));

export function Start() {
  const [error, setError] = useState<string>();
  const [initRoot, setInitRoot] = useState<string | null>(null);
  const [recent, setRecent] = useState<RepoInfo[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);

  const loadProjects = () => closedProjects(setProjects);

  useEffect(() => {
    commands.recentRepos().then((r) => r.status === "ok" && setRecent(r.data));
    closedProjects(setProjects);
  }, []);

  async function openProject(id: string) {
    const r = await commands.openProject(id);
    setError(r.status === "error" ? r.error : undefined);
  }

  async function deleteProject(id: string) {
    const r = await commands.deleteProject(id);
    setError(r.status === "error" ? r.error : undefined);
    loadProjects();
  }

  async function openPath(path: string) {
    const r = await commands.openRepo(path);
    setError(r.status === "error" ? r.error : undefined);
    setInitRoot(r.status === "error" ? await commands.initRoot(path) : null);
  }

  async function initJj(root: string) {
    const r = await commands.initJj(root);
    setInitRoot(null);
    if (r.status === "error") setError(r.error);
    else await openPath(root);
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
      {initRoot ? (
        <div className="init">
          <p>{initRoot} isn't a jj repo.</p>
          <button type="button" onClick={() => initJj(initRoot)}>
            Run jj git init
          </button>
        </div>
      ) : (
        error && <p className="error">{error}</p>
      )}
      {projects.length > 0 && (
        <section className="recent">
          <h2>Projects</h2>
          <ul>
            {projects.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  onClick={() => openProject(p.id)}
                  onContextMenu={contextMenu(() => [
                    { label: "Open", onSelect: () => openProject(p.id) },
                    "-",
                    { label: "Delete Project", onSelect: () => deleteProject(p.id) },
                  ])}
                >
                  <span className="name">{p.name}</span>
                  <span className="root">{p.repos.join(", ")}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
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
