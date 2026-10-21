import type { DockviewApi } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { onAnyShowLook } from "../arch/showLook";
import { bottom } from "../core/bottom";
import { bottomDockOf, dockOf, revealPanel } from "../core/docks";
import { bottomKey, chord, tabKey } from "../core/keys";
import { addTerminal, focusPanel, openReport } from "../core/panels";
import { onAnyReveal } from "../core/reveal";
import { setStateRepo } from "../core/savedState";
import { WorkspaceContext, type WorkspaceName, workspaceKey } from "../core/workspace";
import { DesignFormHost } from "../design/DesignFormHost";
import { requestNewDrawing } from "../drawings/store";
import { confirmClose } from "../editor/unsaved";
import { forgetPr } from "../github/pr";
import { useShortcutsMenu } from "../help/open";
import { commands, type RepoInfo, unwrap } from "../ipc";
import { useFollowWorkingCopy } from "../review/followWorkingCopy";
import { forgetWorkspaceRevset } from "../review/revset";
import { useReviewTarget } from "../review/target";
import { useAtOutside, useTrackWorkingCopy } from "../review/workingCopy";
import { focusLog, openMotions } from "../revisions/loadReview";
import { useSettingsMenu } from "../settings/open";
import { McpOffer } from "../tools/McpOffer";
import { PromptHost, promptKeeps } from "../ui/prompt";
import { selectActivity } from "../workspaces/activity";
import { type AddKind, requestAdd } from "../workspaces/addRequests";
import { useWorkspaces } from "../workspaces/useWorkspaces";
import { WorkspacesSidebar } from "../workspaces/WorkspacesSidebar";
import { AtBanner } from "./AtBanner";
import { BottomPanel } from "./BottomPanel";
import { reopenClosed } from "./closedTabs";
import { JjBlockedBanner, JjBusyTag, useTrackJjBusy } from "./JjBusy";
import { toggleMaximize } from "./maximize";
import { RightPanel } from "./RightPanel";
import { reviewRevset } from "./revsetMenu";
import { SidePanel, type SideState } from "./SidePanel";
import { SideToggle } from "./SideToggle";
import { loadSides, loadWorkspace, type Sides, saveSides, saveWorkspace } from "./sides";
import { useCloseGuard } from "./useCloseGuard";
import { WorkspaceDock } from "./WorkspaceDock";
import { useWordWrapMenu } from "./wordWrap";

/** Shortcuts acting on the selected workspace's panes. */
const shortcuts: Partial<Record<string, (api: DockviewApi) => void>> = {
  "Mod+T": (api) => addTerminal(api),
  "Mod+D": (api) => addTerminal(api, "right"),
  "Mod+Shift+D": (api) => addTerminal(api, "below"),
  // Pane to pane, as in iTerm.
  "Mod+]": (api) => api.moveToNext({ includePanel: true }),
  "Mod+[": (api) => api.moveToPrevious({ includePanel: true }),
};

/** Chords CodeMirror binds (select next occurrence, indent, delete line), left to it while an
 * editor has focus. */
const editorKeys = new Set(["Mod+D", "Mod+]", "Mod+[", "Mod+Shift+K"]);

interface Mounted {
  name: WorkspaceName;
  setup: boolean;
}

/** Side panels around the selected workspace's dock. Each workspace has its own dock
 * and layout. Docks of other workspaces stay mounted but hidden, so their terminals run on. */
export function Workbench({ repo }: { repo: RepoInfo }) {
  setStateRepo(repo.root);
  const [selected, setSelected] = useState<WorkspaceName>(() => loadWorkspace(repo.root));
  const [mounted, setMounted] = useState<Mounted[]>([{ name: selected, setup: false }]);
  const [sides, setSidesState] = useState<Sides>(() => loadSides(repo.root));
  const leftBody = useRef<HTMLDivElement>(null);
  const rightBody = useRef<HTMLDivElement>(null);
  const [bottomEl, setBottomEl] = useState<HTMLDivElement | null>(null);
  const inBottom = () => !!bottomEl?.contains(document.activeElement);
  /** The dock with focus: the bottom one when focus is in it, else the main one. */
  const api = () => (inBottom() ? bottomDockOf(selected) : dockOf(selected));
  useCloseGuard();
  useFollowWorkingCopy();
  useTrackWorkingCopy(selected, useReviewTarget(selected));
  useTrackJjBusy();
  useWordWrapMenu();
  useSettingsMenu(selected);
  useShortcutsMenu(selected);
  useEffect(() => selectActivity(selected), [selected]);

  const selectedRef = useRef(selected);
  selectedRef.current = selected;

  function select(name: WorkspaceName, setup = false) {
    setSelected(name);
    setMounted((m) => (m.some((d) => d.name === name) ? m : [...m, { name, setup }]));
    saveWorkspace(repo.root, name);
  }

  /** Unmounts a deleted workspace's dock, which ends its terminals, and goes to main if it
   * was selected. Reads the selection as it is now: a delete finishes renders after it began. */
  function forget(name: string) {
    if (selectedRef.current === name) select(null);
    forgetPr(name);
    forgetWorkspaceRevset(name);
    setMounted((m) => m.filter((d) => d.name !== name));
  }

  const workspaces = useWorkspaces();
  const atOutside = useAtOutside(selected);
  // The selected workspace, remembered from last time or forgotten from a terminal, may be
  // gone. A list fetched before it was created can lack it too, so a fresh one confirms.
  // biome-ignore lint/correctness/useExhaustiveDependencies: check each list as it loads
  useEffect(() => {
    const name = selectedRef.current;
    const list = workspaces.workspaces;
    if (name === null || !list || list.some((w) => w.name === name)) return;
    unwrap(commands.listWorkspaces()).then(
      (fresh) => {
        if (selectedRef.current === name && !fresh.some((w) => w.name === name)) forget(name);
      },
      () => {},
    );
  }, [workspaces.workspaces]);

  function setSides(update: (s: Sides) => Sides) {
    setSidesState((s) => {
      const next = update(s);
      saveSides(repo.root, next);
      return next;
    });
  }

  const updateSide = (side: keyof Sides, f: (s: SideState) => SideState) =>
    setSides((s) => ({ ...s, [side]: f(s[side]) }));

  const toggle = (side: keyof Sides) => updateSide(side, (s) => ({ ...s, open: !s.open }));

  /** Opens the side panel on `view` and focuses it. */
  function showView(side: keyof Sides, view: string) {
    updateSide(side, (s) => ({ ...s, open: true, view }));
    const body = side === "left" ? leftBody : rightBody;
    requestAnimationFrame(() => body.current?.focus());
  }

  /** Hides the bottom panel when focus is in it or it has no tabs, and otherwise shows and
   * focuses it. Empty, it shows the kinds of tab to open. `terminal` adds a terminal. */
  function toggleBottom(terminal = false) {
    const open = bottom.get().open;
    const empty = !bottomDockOf(selected)?.panels.length;
    if (open && !terminal && (inBottom() || empty)) {
      bottom.set({ open: false });
      focusPanel(dockOf(selected)?.activePanel);
      return;
    }
    // Shown before focusing, since a hidden terminal can't take focus or size itself.
    flushSync(() => bottom.set({ open: true }));
    const dock = bottomDockOf(selected);
    if (!dock) return;
    if (terminal) addTerminal(dock);
    else focusPanel(dock.activePanel);
  }

  useEffect(() => onAnyReveal(() => showView("right", "repo")));
  useEffect(() => onAnyShowLook(() => showView("right", "architecture")));

  /** Closes the focused dock's active tab, asking first about unsaved edits. */
  async function closeTab() {
    const panel = api()?.activePanel;
    if (panel && (await confirmClose(selected, panel.id))) panel.api.close();
  }

  /** Starts adding a workspace or PR in the Workspaces panel, showing it first. */
  function addFromPanel(kind: AddKind) {
    showView("left", "workspaces");
    requestAnimationFrame(() => requestAdd(kind));
  }

  const panelKeys: Partial<Record<string, () => void>> = {
    "Mod+W": () => void closeTab(),
    "Mod+B": () => toggle("left"),
    "Mod+Alt+B": () => toggle("right"),
    "Mod+Shift+K": () => {
      showView("left", "workspaces");
      requestAnimationFrame(() => requestAnimationFrame(focusLog));
    },
    "Mod+Shift+E": () => showView("right", "changes"),
    "Mod+Shift+R": () => showView("right", "repo"),
    "Mod+Shift+M": () => showView("right", "comments"),
    "Mod+Shift+P": () => showView("right", "comments"),
    "Mod+Shift+T": () => reopenClosed(selected),
    "Mod+Shift+O": () => showView("right", "reviewers"),
    "Mod+Shift+I": () => showView("right", "qa"),
    "Mod+Shift+X": () => {
      showView("right", "drawings");
      requestNewDrawing();
    },
    "Mod+J": () => {
      showView("left", "workspaces");
      requestAnimationFrame(() => requestAnimationFrame(openMotions));
    },
    "Mod+Shift+J": () => reviewRevset(selected, "@"),
    "Mod+Shift+ENTER": () => {
      const dock = dockOf(selected);
      if (dock) toggleMaximize(dock, inBottom() ? undefined : dock.activeGroup);
    },
    "Mod+Shift+N": () => addFromPanel("workspace"),
    "Mod+Alt+N": () => addFromPanel("pr"),
    toggle: () => toggleBottom(),
    terminal: () => toggleBottom(true),
    nextTab: () => stepTab(1),
    prevTab: () => stepTab(-1),
    nextPane: () => stepPane(1),
    prevPane: () => stepPane(-1),
  };

  /** Focuses the next or previous tab in the focused pane, wrapping around. */
  function stepTab(by: 1 | -1) {
    const group = api()?.activeGroup;
    const at = group?.activePanel ? group.panels.indexOf(group.activePanel) : -1;
    if (!group || at < 0) return;
    focusPanel(group.panels[(at + by + group.panels.length) % group.panels.length]);
  }

  /** Focuses the next or previous pane in the focused dock, wrapping around. */
  function stepPane(by: 1 | -1) {
    const dock = api();
    if (!dock) return;
    if (by > 0) dock.activateNext();
    else dock.activatePrevious();
    focusPanel(dock.activeGroup?.activePanel);
  }

  /** The action for a chord, if it has one now. */
  const keyRef = useRef<(key: string) => (() => void) | undefined>(undefined);
  keyRef.current = (key) => {
    const action = shortcuts[key];
    const target = api();
    return panelKeys[key] ?? (action && target ? () => action(target) : undefined);
  };

  useEffect(() => {
    // Capture phase, so app shortcuts win over whatever panel has focus, including xterm.
    const onKey = (e: KeyboardEvent) => {
      if (promptKeeps(e)) return;
      const key = bottomKey(e) ?? tabKey(e) ?? chord(e) ?? "";
      if (editorKeys.has(key) && (e.target as Element).closest?.(".cm-editor")) return;
      const run = keyRef.current?.(key);
      if (!run) return;
      e.preventDefault();
      e.stopPropagation();
      run();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const withApi = (f: (api: DockviewApi) => void) => () => {
    const a = api();
    if (a) f(a);
  };

  return (
    <div className="workbench">
      <header className={`toolbar${atOutside ? " warn" : ""}`}>
        <SideToggle
          side="left"
          open={sides.left.open}
          shortcut="⌘B"
          onClick={() => toggle("left")}
        />
        <span className="repo" title={repo.root}>
          {repo.name}
          <span className="repo-sep">/</span>
          <span className="repo-workspace">{selected ?? "main"}</span>
        </span>
        <JjBusyTag workspace={selected} />
        <span className="spacer" />
        <SideToggle
          side="right"
          open={sides.right.open}
          shortcut="⌥⌘B"
          onClick={() => toggle("right")}
        />
      </header>
      <AtBanner workspace={selected} />
      <JjBlockedBanner workspace={selected} />
      <McpOffer />
      <PromptHost />
      <DesignFormHost workspace={selected} />
      <div className="workbench-body">
        <SidePanel
          side="left"
          state={sides.left}
          onChange={(left) => updateSide("left", () => left)}
          bodyRef={leftBody}
          views={[
            {
              id: "workspaces",
              title: "Workspaces",
              icon: "folder",
              render: () => (
                <WorkspacesSidebar
                  list={workspaces}
                  selected={selected}
                  onSelect={select}
                  onDeleted={forget}
                  onHide={() => toggle("left")}
                />
              ),
            },
          ]}
        />
        <div className="workbench-center">
          <div className="docks">
            {mounted.map((d) => (
              <WorkspaceDock
                key={workspaceKey(d.name)}
                workspace={d.name}
                hidden={d.name !== selected}
                setup={d.setup}
                bottomEl={bottomEl}
              />
            ))}
          </div>
          <BottomPanel docksRef={setBottomEl} workspace={selected} />
        </div>
        <WorkspaceContext.Provider value={selected}>
          <RightPanel
            state={sides.right}
            onChange={(right) => updateSide("right", () => right)}
            bodyRef={rightBody}
            onOpenReport={(run) => {
              if (!revealPanel(selected, `report-${run.id}`))
                withApi((a) => openReport(a, run.id))();
            }}
            onHide={() => toggle("right")}
          />
        </WorkspaceContext.Provider>
      </div>
    </div>
  );
}
