import { open } from "@tauri-apps/plugin-dialog";
import { Fragment, useEffect, useState } from "react";
import type { ProjectRepo, WorkspaceDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { confirmWorkspace } from "../editor/unsaved";
import { frameRepo } from "../frame/boot";
import { type ShellState, shell, useShell } from "../frame/shell";
import { addRepo } from "../frame/useProjectFrame";
import { OpenPr } from "../github/OpenPr";
import { reloadPr } from "../github/pr";
import { commands, errorText, unwrap } from "../ipc";
import { RevisionsSection } from "../revisions/RevisionsSection";
import { useGithubOff } from "../tools/agents";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { confirmShown, promptText, showText } from "../ui/prompt";
import { SectionHeader } from "../ui/SectionHeader";
import { type AddKind, onAddRequest, onAsk } from "./addRequests";
import { ForeignRow } from "./ForeignRow";
import { arrange, forgetPin, usePins } from "./pins";
import { RepoRow } from "./RepoRow";
import { usePinDrag } from "./usePinDrag";
import { useTopSplit } from "./useTopSplit";
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
  /** Puts a workspace to sleep. */
  onSleep(name: WorkspaceName): void;
  /** Hides the left panel. */
  onHide(): void;
}

/** The repo's workspaces, and the selected one's revisions under them. Click a workspace to
 * switch to it. */
export function WorkspacesSidebar({ list, selected, onSelect, onDeleted, onSleep, onHide }: Props) {
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
  const split = useTopSplit();
  const { drop, start: startDrag } = usePinDrag(pins.pinned);

  function start(kind: AddKind) {
    if (kind === "workspace") setNaming(true);
    else if (!githubOff) setOpeningPr(true);
  }
  // Resubscribed each render, so the shortcut sees the current state.
  useEffect(() => onAddRequest(start));
  useEffect(() =>
    onAsk(({ kind, ws }) => {
      const w = workspaces?.find((x) => x.name === ws);
      if (kind === "delete") setConfirming(ws);
      else if (w) locate(w);
    }),
  );
  const project = useShell();

  /** Asks where a workspace's files are, then opens it there. */
  async function locate(w: WorkspaceDto) {
    if (w.name === null) return;
    const root = await open({ directory: true, defaultPath: w.root || undefined });
    if (typeof root !== "string") return;
    try {
      await unwrap(commands.locateWorkspace(w.name, root));
      await reload();
      onSelect(w.name);
    } catch (e) {
      setError(errorText(e));
    }
  }

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

  const addItems: CtxEntry[] = [
    { label: "New Workspace…", keys: "⇧⌘N", onSelect: () => start("workspace") },
    {
      label: "Review a PR by URL or Number…",
      keys: "⌥⌘N",
      disabled: githubOff,
      onSelect: () => start("pr"),
    },
    project && ("-" as const),
    project && { label: "Add Repository…", keys: "⌘O", onSelect: () => addRepo(setError) },
  ];

  async function rename() {
    const sh = shell();
    if (!sh || !project) return;
    const next = await promptText("Rename Project", {
      value: project.project.name,
      action: "Rename",
    });
    const e = next && (await sh.rename(next));
    if (e) setError(e);
  }

  /** The menu of the header and the space around the rows. Text fields keep their own. */
  function menu(e: React.MouseEvent) {
    if ((e.target as Element).closest("input, textarea")) return;
    openMenu(e, [
      ...addItems,
      "-",
      { label: "Refresh", onSelect: refresh },
      project && { label: "Rename Project…", onSelect: rename },
      { label: "Hide Left Panel", keys: "⌘B", onSelect: onHide },
    ]);
  }

  const { top, rest } = arrange(workspaces ?? [], (w) => workspaceKey(w.name), pins.pinned);
  // One workspace stays awake.
  const canSleep = (workspaces ?? []).filter((w) => !w.asleep).length > 1;
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
        onSelect={() => (w.missing ? locate(w) : onSelect(w.name))}
        onSleep={canSleep ? () => onSleep(w.name) : undefined}
        onLocate={() => locate(w)}
        onDelete={() => setConfirming(w.name ?? undefined)}
        onPinDrag={pinned ? (e) => startDrag(e, key) : undefined}
        drop={pinned ? dropAt(key) : undefined}
        nested={!!project}
      />
    );
  }

  /** Pinned workspaces, a hair, then the rest, under their repo. */
  const nestedRows = (pinned: React.ReactNode[], others: React.ReactNode[]) => (
    <>
      {pinned}
      {pinned.length > 0 && others.length > 0 && <li className="ws-sub" />}
      {others}
    </>
  );

  function repoItems(r: ProjectRepo, s: ShellState) {
    const open = !r.asleep && !s.ui.collapsed.includes(r.label);
    const tree = s.trees[r.label];
    const foreign = (x: (typeof tree.top)[number]) => (
      <ForeignRow key={workspaceKey(x.w.name)} row={x} label={r.label} />
    );
    return (
      <Fragment key={r.label}>
        <RepoRow repo={r} s={s} onError={setError} />
        {open &&
          (r.label === frameRepo
            ? nestedRows(
                top.map((w) => row(w, true)),
                rest.map((w) => row(w, false)),
              )
            : tree && nestedRows(tree.top.map(foreign), tree.rest.map(foreign)))}
      </Fragment>
    );
  }

  /** The project's repos, pinned ones first, each with its workspaces under it. */
  function tree(s: ShellState) {
    const repos = arrange(s.project.repos, (r) => r.label, s.ui.pinned);
    return (
      <Pinned
        top={repos.top.map((r) => repoItems(r, s))}
        rest={<ul>{repos.rest.map((r) => repoItems(r, s))}</ul>}
      />
    );
  }

  return (
    <div className="workspaces">
      <div ref={split.top} className="ws-top" style={split.style} onContextMenu={menu}>
        <SectionHeader title={project?.project.name ?? "Workspaces"}>
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
        {project ? (
          tree(project)
        ) : (
          <Pinned
            top={top.map((w) => row(w, true))}
            rest={<ul>{rest.map((w) => row(w, false))}</ul>}
          />
        )}
      </div>
      <div className="ws-split" {...split.splitter}>
        <span />
      </div>
      <RevisionsSection workspace={selected} refreshed={refreshed} />
    </div>
  );
}

/** Pinned rows under a heading and a hair, then the rest. */
function Pinned({ top, rest }: { top: React.ReactNode[]; rest: React.ReactNode }) {
  return (
    <>
      {top.length > 0 && (
        <>
          <div className="ws-cap">Pinned</div>
          <ul>{top}</ul>
          <div className="ws-divider" />
        </>
      )}
      {rest}
    </>
  );
}
