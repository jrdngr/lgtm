import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  themeDark,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import "./dock.css";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useAgentArch } from "../arch/useAgentArch";
import { ReportPanel } from "../bots/ReportPanel";
import { useAgentReports } from "../bots/useAgentReports";
import { bottom } from "../core/bottom";
import { type DockSlot, registerDock } from "../core/docks";
import { addTerminal, defaultLayout, focusPanel, isReviewPanel } from "../core/panels";
import { WorkspaceContext, type WorkspaceName } from "../core/workspace";
import { EditorPanel } from "../editor/EditorPanel";
import { useAgentShow } from "../editor/useAgentShow";
import { PrPreview } from "../github/PrPreview";
import { ShortcutsPanel } from "../help/ShortcutsPanel";
import { commands, unwrap } from "../ipc";
import { QaTab } from "../qa/QaTab";
import { useAgentQa } from "../qa/useAgentQa";
import { WorkspaceReview } from "../review/IdleReview";
import { ReviewPanel } from "../review/ReviewPanel";
import { SettingsPanel } from "../settings/SettingsPanel";
import { ArchitecturePanel } from "../system/ArchitecturePanel";
import { useAgentSystem } from "../system/useAgentSystem";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { guarded } from "../ui/Crash";
import { trackClosed } from "./closedTabs";
import { isMoving, takeTabsFrom } from "./crossDock";
import { EmptyGroup } from "./EmptyGroup";
import { savedFolds, trackFolds } from "./fold";
import { NewTab, SplitButtons } from "./GroupActions";
import { parseLayout, restoreLayout, type SavedLayout } from "./layout";
import { Tab } from "./Tab";

const components = {
  review: guarded(ReviewPanel),
  terminal: guarded(TerminalPanel),
  report: guarded(ReportPanel),
  architecture: guarded(ArchitecturePanel),
  editor: guarded(EditorPanel),
  prPreview: guarded(PrPreview),
  settings: guarded(SettingsPanel),
  shortcuts: guarded(ShortcutsPanel),
  qa: guarded(QaTab),
  // Restores layouts from earlier builds, whose Tasks, Files, Design, Threads, Graph and Agent
  // runs panels are then closed, and whose System tab becomes Architecture.
  system: () => null,
  tasks: () => null,
  threads: () => null,
  files: () => null,
  design: () => null,
  graph: () => null,
  runs: () => null,
};

interface Props {
  workspace: WorkspaceName;
  hidden: boolean;
  /** A new workspace: its first terminal runs `.lgtm/setup`. */
  setup: boolean;
  /** Where the workspace's bottom dock renders, once the bottom panel is up. */
  bottomEl?: HTMLElement | null;
}

/** One workspace's panes, in its main dock and its bottom dock, with their own saved layout.
 * Hidden docks stay mounted so their terminals keep running. */
export function WorkspaceDock({ workspace, hidden, setup, bottomEl }: Props) {
  const timer = useRef<number>(undefined);
  const gone = useRef(false);
  const shown = useRef(!hidden);
  shown.current = !hidden;
  /** Each dock's latest api. React can mount a dock twice, and panes added to the first,
   * already disposed api would stay mounted outside the dock. */
  const latest = useRef<Partial<Record<DockSlot, DockviewApi>>>({});
  const loading = useRef<Promise<SavedLayout>>(undefined);
  const loaded = useRef<SavedLayout>(undefined);
  /** Unknown until the layout is restored. */
  const [hasPane, setHasPane] = useState<boolean>();
  useAgentShow(workspace);
  useAgentQa(workspace);
  useAgentReports(workspace);
  useAgentArch(workspace);
  useAgentSystem(workspace);
  // A deleted workspace's layout must not be saved again after its file is removed.
  useEffect(() => {
    gone.current = false;
    return () => {
      gone.current = true;
      clearTimeout(timer.current);
      registerDock(workspace, undefined);
      registerDock(workspace, undefined, "bottom");
    };
  }, [workspace]);

  /** Hides the bottom panel when this workspace is shown with nothing in its bottom dock. It
   * opens empty only when asked to. */
  function hideEmpty() {
    const below = latest.current.bottom;
    if (shown.current && below && below.panels.length === 0 && bottom.get().open)
      bottom.set({ open: false });
  }
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the workspace is shown
  useEffect(() => {
    if (!hidden) hideEmpty();
  }, [hidden]);

  /** The saved layout, loaded once for both docks. */
  function load() {
    loading.current ??= unwrap(commands.loadLayout(workspace))
      .then(parseLayout)
      .catch(() => ({}))
      .then((saved) => {
        loaded.current = saved;
        return saved;
      });
    return loading.current;
  }

  /** Saves both docks. A dock that isn't up yet keeps what was saved for it. */
  function save() {
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      const saved = loaded.current;
      if (gone.current || !saved) return;
      const { main, bottom: below } = latest.current;
      const layout: SavedLayout = {
        main: main ? main.toJSON() : saved.main,
        bottom: below ? (below.panels.length ? below.toJSON() : undefined) : saved.bottom,
        folds: {
          main: main ? savedFolds(main) : saved.folds?.main,
          bottom: below ? savedFolds(below) : saved.folds?.bottom,
        },
      };
      commands.saveLayout(workspace, JSON.stringify(layout));
    }, 500);
  }

  async function readyMain({ api }: DockviewReadyEvent) {
    latest.current.main = api;
    const saved = await load();
    if (latest.current.main !== api) return;
    restoreLayout(api, saved.main, () => defaultLayout(api));
    trackFolds(api, saved.folds?.main);
    const panes = () => setHasPane(api.panels.some(isReviewPanel));
    panes();
    api.onDidAddPanel(panes);
    api.onDidRemovePanel(panes);
    api.onDidLayoutChange(save);
    takeTabsFrom(api, () => latest.current.bottom, workspace);
    trackClosed(workspace, api, "main");
    registerDock(workspace, api);
  }

  async function readyBottom({ api }: DockviewReadyEvent) {
    latest.current.bottom = api;
    const saved = await load();
    if (latest.current.bottom !== api) return;
    restoreLayout(api, saved.bottom);
    trackFolds(api, saved.folds?.bottom);
    // A new workspace's setup script runs in a terminal of its own.
    if (setup && api.panels.length === 0) addTerminal(api, undefined, true);
    hideEmpty();
    // Closing the last tab hides the panel, while dragging it to the main dock leaves the panel
    // open. Checked once the close settles, since moving a tab within the dock removes it from
    // its pane before adding it to the next.
    api.onDidRemovePanel((panel) => {
      if (isMoving(panel)) return;
      setTimeout(() => {
        if (gone.current || !shown.current || api.panels.length > 0 || !bottom.get().open) return;
        api.clear();
        bottom.set({ open: false });
        focusPanel(latest.current.main?.activePanel);
      });
    });
    api.onDidLayoutChange(save);
    takeTabsFrom(api, () => latest.current.main, workspace);
    trackClosed(workspace, api, "bottom");
    registerDock(workspace, api, "bottom");
  }

  const dock = (ready: (e: DockviewReadyEvent) => void) => (
    <div className="dock" hidden={hidden}>
      <DockviewReact
        components={components}
        defaultTabComponent={Tab}
        leftHeaderActionsComponent={NewTab}
        rightHeaderActionsComponent={SplitButtons}
        watermarkComponent={EmptyGroup}
        onReady={ready}
        theme={themeDark}
        // WebKit's HTML drag and drop sometimes drops nothing and leaves the drop highlight up.
        dndStrategy="pointer"
      />
    </div>
  );

  return (
    <WorkspaceContext.Provider value={workspace}>
      {dock(readyMain)}
      {bottomEl && createPortal(dock(readyBottom), bottomEl)}
      {hasPane !== undefined && <WorkspaceReview workspace={workspace} hasPane={hasPane} />}
    </WorkspaceContext.Provider>
  );
}
