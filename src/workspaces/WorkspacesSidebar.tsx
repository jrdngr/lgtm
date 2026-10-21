import { useEffect, useState } from "react";
import type { WorkspaceDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { confirmWorkspace } from "../editor/unsaved";
import { OpenPr } from "../github/OpenPr";
import { reloadPr } from "../github/pr";
import { commands } from "../ipc";
import { RevisionsSection } from "../revisions/RevisionsSection";
import { useGithubOff } from "../tools/agents";
import { openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { confirmShown, showText } from "../ui/prompt";
import { SectionHeader } from "../ui/SectionHeader";
import { type AddKind, onAddRequest } from "./addRequests";
import { arrange, forgetPin, usePins } from "./pins";
import { usePinDrag } from "./usePinDrag";
import type { Workspaces } from "./useWorkspaces";
import { WorkspaceRow } from "./WorkspaceRow";
import "./workspaces.css";

interface Props {
  /** The repo's workspaces, from `useWorkspaces`. */
  list: Workspaces;
  selected: WorkspaceName;
  /** `setup` is set for a new workspace with a `.lgtm/setup`, which its first terminal runs
   * once approved. */
  onSelect(name: WorkspaceName, setup?: boolean): void;
  onDeleted(name: string): void;
  /** Hides the left panel. */
  onHide(): void;
}

/** The repo's workspaces, and the selected one's revisions under them. Click a workspace to
 * switch to it. */
export function WorkspacesSidebar({ list, selected, onSelect, onDeleted, onHide }: Props) {
  const { workspaces, error, setError, reload } = list;
  const [naming, setNaming] = useState(false);
  const [openingPr, setOpeningPr] = useState(false);
  const githubOff = useGithubOff();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState<string>();
  // Bumped by Refresh, so the revisions' PRs ask GitHub again.
  const [refreshed, setRefreshed] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const pins = usePins();
  const { drop, start: startDrag } = usePinDrag(pins.pinned);

  function start(kind: AddKind) {
    if (kind === "workspace") setNaming(true);
    else if (!githubOff) setOpeningPr(true);
  }
  // Resubscribed each render, so the shortcut sees the current state.
  useEffect(() => onAddRequest(start));

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
    // Skipped, the terminal opens as a plain shell: the backend runs only approved scripts.
    const setup = r.data;
    if (
      setup.kind === "unapproved" &&
      (await confirmShown(".lgtm/setup", setup.script, "Run", "Skip"))
    ) {
      const ok = await commands.approveSetup(ws, setup.hash);
      if (ok.status === "error") setError(ok.error);
    }
    if (setup.kind === "unshowable") {
      await showText(".lgtm/setup", setup.why === "tooLarge" ? "Too large" : "Not text", "Skip");
    }
    onSelect(ws, setup.kind !== "none");
  }

  async function prOpened(ws: string) {
    setOpeningPr(false);
    setError(undefined);
    await Promise.all([reload(), reloadPr(ws)]);
    onSelect(ws);
  }

  async function remove(ws: string) {
    setConfirming(undefined);
    if (!(await confirmWorkspace(ws))) return;
    setError(undefined);
    const r = await commands.deleteWorkspace(ws);
    if (r.status === "error") setError(r.error);
    else {
      forgetPin(workspaceKey(ws));
      onDeleted(ws);
    }
    reload();
  }

  async function refresh() {
    setRefreshing(true);
    setError(undefined);
    setRefreshed((n) => n + 1);
    await Promise.all([reload(), reloadPr(selected)]);
    setRefreshing(false);
  }

  const addItems = [
    { label: "New Workspace…", keys: "⇧⌘N", onSelect: () => start("workspace") },
    {
      label: "Review a PR by URL or Number…",
      keys: "⌥⌘N",
      disabled: githubOff,
      onSelect: () => start("pr"),
    },
  ];

  /** The menu of the header and the space around the rows. Text fields keep their own. */
  function menu(e: React.MouseEvent) {
    if ((e.target as Element).closest("input, textarea")) return;
    openMenu(e, [
      ...addItems,
      "-",
      { label: "Refresh", onSelect: refresh },
      { label: "Hide Left Panel", keys: "⌘B", onSelect: onHide },
    ]);
  }

  const { top, rest } = arrange(workspaces ?? [], (w) => workspaceKey(w.name), pins.pinned);
  const lastPin = top.filter((w) => workspaceKey(w.name) !== drop?.key).at(-1);

  /** Where the dragged pin would land, next to the row with `key`. */
  function dropAt(key: string) {
    if (!drop || key === drop.key) return undefined;
    if (drop.before === key) return "before";
    if (drop.before === null && lastPin && workspaceKey(lastPin.name) === key) return "after";
    return undefined;
  }

  function row(w: WorkspaceDto, pinned: boolean) {
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
        selected={selected === w.name}
        onSelect={() => onSelect(w.name)}
        onDelete={() => setConfirming(w.name ?? undefined)}
        onPinDrag={pinned ? (e) => startDrag(e, key) : undefined}
        drop={pinned ? dropAt(key) : undefined}
      />
    );
  }

  return (
    <div className="workspaces">
      <div className="ws-top" onContextMenu={menu}>
        <SectionHeader title="Workspaces">
          <IconButton
            icon="sync"
            label="Refresh"
            disabled={refreshing}
            onClick={(e) => {
              e.stopPropagation();
              refresh();
            }}
          />
          <button
            type="button"
            className="ws-new"
            onClick={(e) => {
              e.stopPropagation();
              openMenu(e, addItems);
            }}
          >
            + New ▾
          </button>
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
        {top.length > 0 && (
          <>
            <div className="ws-cap">Pinned</div>
            <ul>{top.map((w) => row(w, true))}</ul>
            <div className="ws-divider" />
          </>
        )}
        <ul>{rest.map((w) => row(w, false))}</ul>
      </div>
      <RevisionsSection workspace={selected} refreshed={refreshed} />
    </div>
  );
}
