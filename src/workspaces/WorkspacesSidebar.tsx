import { useEffect, useRef, useState } from "react";
import { OpenPr } from "../github/OpenPr";
import { reloadPr, usePr } from "../github/pr";
import { commands } from "../ipc";
import { Count } from "../review/FilesPanel";
import { useGithubOff } from "../tools/agents";
import { type WorkspaceName, workspaceKey } from "./context";
import { NearbyPrs } from "./NearbyPrs";
import { useWorkspaces } from "./useWorkspaces";
import "./workspaces.css";

interface Props {
  selected: WorkspaceName;
  /** `setup` is set for a new workspace whose first terminal runs `.lgtm/setup`. */
  onSelect(name: WorkspaceName, setup?: boolean): void;
  onDeleted(name: string): void;
  /** The selected workspace isn't in the repo any more. */
  onMissing(): void;
}

/** A PR workspace's PR, in place of its working copy's description. */
function PrTitle({ name, fallback }: { name: string; fallback: string }) {
  const pr = usePr(name);
  return <>{pr ? `#${pr.number} ${pr.title}` : fallback}</>;
}

/** The repo's jj workspaces with review progress. Click one to switch to it. */
export function WorkspacesSidebar({ selected, onSelect, onDeleted, onMissing }: Props) {
  const { workspaces, progress, error, setError, reload } = useWorkspaces();
  const [naming, setNaming] = useState(false);
  const [openingPr, setOpeningPr] = useState(false);
  const githubOff = useGithubOff();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState<string>();

  // A workspace remembered from last time may have been forgotten since.
  const checked = useRef(false);
  useEffect(() => {
    if (!workspaces || checked.current) return;
    checked.current = true;
    if (!workspaces.some((w) => w.name === selected)) onMissing();
  }, [workspaces, selected, onMissing]);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    const ws = name.trim();
    if (!ws) return;
    setBusy(true);
    const r = await commands.createWorkspace(ws);
    setBusy(false);
    if (r.status === "error") {
      setError(r.error);
      return;
    }
    setNaming(false);
    setName("");
    await reload();
    onSelect(ws, r.data);
  }

  async function prOpened(ws: string, setup: boolean) {
    setOpeningPr(false);
    setError(undefined);
    await Promise.all([reload(), reloadPr(ws)]);
    onSelect(ws, setup);
  }

  async function remove(ws: string) {
    setConfirming(undefined);
    onDeleted(ws);
    const r = await commands.deleteWorkspace(ws);
    if (r.status === "error") setError(r.error);
    reload();
  }

  return (
    <div className="workspaces">
      <div className="workspaces-bar">
        <span className="spacer" />
        <button
          type="button"
          onClick={() => setOpeningPr((o) => !o)}
          disabled={githubOff}
          aria-pressed={openingPr}
          title={
            githubOff ? "Needs the GitHub CLI (gh)" : "Review a GitHub PR in its own workspace"
          }
        >
          PR
        </button>
        <button type="button" onClick={() => setNaming(true)} disabled={naming}>
          New
        </button>
      </div>
      {openingPr && (
        <OpenPr onClose={() => setOpeningPr(false)} onError={setError} onOpened={prOpened} />
      )}
      {naming && (
        <form className="workspace-form" onSubmit={create}>
          <input
            // biome-ignore lint/a11y/noAutofocus: the form opens to take a name
            autoFocus
            value={name}
            placeholder="workspace-name"
            spellCheck={false}
            aria-label="Workspace name"
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setNaming(false)}
          />
          <button type="submit" disabled={busy || !name.trim()}>
            {busy ? "Creating" : "Create"}
          </button>
        </form>
      )}
      {error && <div className="review-error">{error}</div>}
      <ul>
        {workspaces?.map((w) => {
          const key = workspaceKey(w.name);
          const p = progress.get(key);
          if (confirming === w.name) {
            return (
              <li key={key} className="workspace confirm">
                <span>Delete {w.title}? Its commits stay in the repo.</span>
                <button type="button" onClick={() => remove(confirming)}>
                  Delete
                </button>
                <button type="button" onClick={() => setConfirming(undefined)}>
                  Cancel
                </button>
              </li>
            );
          }
          return (
            <li key={key} className={`workspace${selected === w.name ? " selected" : ""}`}>
              <button
                type="button"
                className="workspace-open"
                onClick={() => onSelect(w.name)}
                title={w.root}
              >
                <span className="workspace-title">
                  {w.title}
                  {w.name === null && <span className="workspace-main">main</span>}
                </span>
                <span className="workspace-desc">
                  {w.name?.startsWith("pr-") ? (
                    <PrTitle name={w.name} fallback={`${w.changeId} ${w.description}`} />
                  ) : (
                    `${w.changeId} ${w.description || "(no description)"}`
                  )}
                </span>
                {p && (
                  <span className="workspace-progress" title={`Progress of ${p.revset}`}>
                    <Count p={p} />
                  </span>
                )}
              </button>
              {w.name !== null && (
                <button
                  type="button"
                  className="workspace-delete"
                  title="Delete workspace"
                  onClick={() => setConfirming(w.name ?? undefined)}
                >
                  ✕
                </button>
              )}
            </li>
          );
        })}
      </ul>
      <NearbyPrs workspace={selected} onOpened={prOpened} onError={setError} />
    </div>
  );
}
