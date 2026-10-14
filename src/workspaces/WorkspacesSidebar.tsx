import { useState } from "react";
import { confirmWorkspace } from "../editor/unsaved";
import { OpenPr } from "../github/OpenPr";
import { reloadPr } from "../github/pr";
import { commands } from "../ipc";
import { useGithubOff } from "../tools/agents";
import { openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { SectionHeader } from "../ui/SectionHeader";
import { type WorkspaceName, workspaceKey } from "./context";
import { NearbyPrs } from "./NearbyPrs";
import type { Workspaces } from "./useWorkspaces";
import { WorkspaceRow } from "./WorkspaceRow";
import "./workspaces.css";

interface Props {
  /** The repo's workspaces, from `useWorkspaces`. */
  list: Workspaces;
  selected: WorkspaceName;
  /** `setup` is set for a new workspace whose first terminal runs `.lgtm/setup`. */
  onSelect(name: WorkspaceName, setup?: boolean): void;
  onDeleted(name: string): void;
  /** Hides the left panel. */
  onHide(): void;
}

/** The repo's workspaces with review progress. Click one to switch to it. */
export function WorkspacesSidebar({ list, selected, onSelect, onDeleted, onHide }: Props) {
  const { workspaces, progress, error, setError, reload } = list;
  const [naming, setNaming] = useState(false);
  const [openingPr, setOpeningPr] = useState(false);
  const githubOff = useGithubOff();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState<string>();

  async function create(e: React.FormEvent) {
    e.preventDefault();
    const ws = name.trim();
    if (!ws) return;
    setBusy(true);
    setError(undefined);
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
    if (!(await confirmWorkspace(ws))) return;
    setError(undefined);
    const r = await commands.deleteWorkspace(ws);
    if (r.status === "error") setError(r.error);
    else onDeleted(ws);
    reload();
  }

  /** The menu of the header and the space around the rows. Text fields keep their own. */
  function menu(e: React.MouseEvent) {
    if ((e.target as Element).closest("input, textarea")) return;
    openMenu(e, [
      { label: "New Workspace…", onSelect: () => setNaming(true) },
      { label: "Review a GitHub PR…", disabled: githubOff, onSelect: () => setOpeningPr(true) },
      "-",
      { label: "Hide Left Panel", keys: "⌘B", onSelect: onHide },
    ]);
  }

  return (
    <div className="workspaces" onContextMenu={menu}>
      <SectionHeader title="Workspaces">
        <IconButton
          icon="pr"
          label={
            githubOff ? "Needs the GitHub CLI (gh)" : "Review a GitHub PR in its own workspace"
          }
          onClick={() => setOpeningPr((o) => !o)}
          disabled={githubOff}
          aria-pressed={openingPr}
        />
        <IconButton
          icon="plus"
          label="New workspace"
          onClick={() => setNaming(true)}
          disabled={naming}
        />
      </SectionHeader>
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
          <button type="submit" className="primary" disabled={busy || !name.trim()}>
            {busy ? "Creating" : "Create"}
          </button>
        </form>
      )}
      {error && <div className="review-error">{error}</div>}
      <ul>
        {workspaces?.map((w) => {
          const key = workspaceKey(w.name);
          if (confirming === w.name) {
            return (
              <li key={key} className="workspace confirm">
                <span>Delete {w.title}? Its commits stay in the repo.</span>
                <button type="button" onClick={() => remove(confirming)}>
                  Delete
                </button>
                <button type="button" className="quiet" onClick={() => setConfirming(undefined)}>
                  Cancel
                </button>
              </li>
            );
          }
          return (
            <WorkspaceRow
              key={key}
              w={w}
              progress={progress.get(key)}
              selected={selected === w.name}
              onSelect={() => onSelect(w.name)}
              onDelete={() => setConfirming(w.name ?? undefined)}
            />
          );
        })}
      </ul>
      <NearbyPrs workspace={selected} onOpened={prOpened} onError={setError} />
    </div>
  );
}
