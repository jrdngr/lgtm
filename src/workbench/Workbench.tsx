import type { DockviewApi } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { onAnyReveal } from "../editor/reveal";
import { confirmClose } from "../editor/unsaved";
import { useShortcutsMenu } from "../help/open";
import { commands, type RepoInfo, unwrap } from "../ipc";
import { useFollowWorkingCopy } from "../review/followWorkingCopy";
import { openRevisions } from "../review/target";
import { useSettingsMenu } from "../settings/open";
import { McpOffer } from "../tools/McpOffer";
import { PromptHost } from "../ui/prompt";
import { selectActivity } from "../workspaces/activity";
import { type AddKind, requestAdd } from "../workspaces/addRequests";
import { WorkspaceContext, type WorkspaceName, workspaceKey } from "../workspaces/context";
import { useWorkspaces } from "../workspaces/useWorkspaces";
import { WorkspacesSidebar } from "../workspaces/WorkspacesSidebar";
import { BottomPanel } from "./BottomPanel";
import { bottom } from "./bottom";
import { reopenClosed } from "./closedTabs";
import { bottomDockOf, dockOf, revealPanel } from "./docks";
import { bottomKey, chord } from "./keys";
import { addTerminal, focusPanel, openReport } from "./panels";
import { RevPicker } from "./RevPicker";
import { RightPanel } from "./RightPanel";
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
  useFollowWorkingCopy();
  useWordWrapMenu();
  useSettingsMenu(selected);
  useShortcutsMenu(selected);
  useEffect(() => selectActivity(selected), [selected]);

  function select(name: WorkspaceName, setup = false) {
    setSelected(name);
    setMounted((m) => (m.some((d) => d.name === name) ? m : [...m, { name, setup }]));
    saveWorkspace(repo.root, name);
  }

  /** Unmounts a deleted workspace's dock, which ends its terminals. */
  function forget(name: string) {
    if (selected === name) select(null);
    setMounted((m) => m.filter((d) => d.name !== name));
  }

  const workspaces = useWorkspaces();
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
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
   * focuses it. It opens with a terminal when it would open empty, and `terminal` adds one. */
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
    if (terminal || empty) addTerminal(dock);
    else focusPanel(dock.activePanel);
  }

  useEffect(() => onAnyReveal(() => showView("right", "repo")));

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
    "Mod+Shift+K": () => showView("left", "workspaces"),
    "Mod+Shift+E": () => showView("right", "changes"),
    "Mod+Shift+Y": () => showView("right", "symbols"),
    "Mod+Shift+R": () => showView("right", "repo"),
    "Mod+Shift+M": () => showView("right", "comments"),
    "Mod+Shift+P": () => showView("right", "pr"),
    "Mod+Shift+T": () => reopenClosed(selected),
    "Mod+Shift+H": () => showView("right", "walkthrough"),
    "Mod+Shift+O": () => showView("right", "reviewers"),
    "Mod+Shift+I": () => showView("right", "qa"),
    "Mod+J": () => openRevisions(selected),
    "Mod+Shift+N": () => addFromPanel("workspace"),
    "Mod+Alt+N": () => addFromPanel("pr"),
    toggle: () => toggleBottom(),
    terminal: () => toggleBottom(true),
  };

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
      const key = bottomKey(e) ?? chord(e) ?? "";
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
      <header className="toolbar">
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
        <span className="spacer" />
        <div className="toolbar-center">
          <RevPicker workspace={selected} />
        </div>
        <SideToggle
          side="right"
          open={sides.right.open}
          shortcut="⌥⌘B"
          onClick={() => toggle("right")}
        />
      </header>
      <McpOffer />
      <PromptHost />
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
          <BottomPanel docksRef={setBottomEl} />
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
