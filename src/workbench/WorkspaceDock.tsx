import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  themeDark,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import "./dock.css";
import { useEffect, useRef, useState } from "react";
import { createPortal, flushSync } from "react-dom";
import { RunsPanel } from "../agents/RunsPanel";
import { ReportPanel } from "../bots/ReportPanel";
import { EditorPanel } from "../editor/EditorPanel";
import { useAgentShow } from "../editor/useAgentShow";
import { PrPreview } from "../github/PrPreview";
import { GraphPanel } from "../graph/GraphPanel";
import { ShortcutsPanel } from "../help/ShortcutsPanel";
import { commands, unwrap } from "../ipc";
import { QaTab } from "../qa/QaTab";
import { WorkspaceReview } from "../review/IdleReview";
import { ReviewPanel } from "../review/ReviewPanel";
import { SettingsPanel } from "../settings/SettingsPanel";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { ThreadsPanel } from "../threads/ThreadsPanel";
import { WorkspaceContext, type WorkspaceName } from "../workspaces/context";
import { bottom } from "./bottom";
import { trackClosed } from "./closedTabs";
import { isMoving, takeTabsFrom } from "./crossDock";
import { type DockSlot, registerDock } from "./docks";
import { EmptyGroup } from "./EmptyGroup";
import { NewTab, SplitButtons } from "./GroupActions";
import { parseLayout, restoreLayout, type SavedLayout } from "./layout";
import { addTerminal, defaultLayout, focusPanel, isReviewPanel } from "./panels";
import { Tab } from "./Tab";

const components = {
  review: ReviewPanel,
  terminal: TerminalPanel,
  report: ReportPanel,
  graph: GraphPanel,
  editor: EditorPanel,
  prPreview: PrPreview,
  settings: SettingsPanel,
  shortcuts: ShortcutsPanel,
  threads: ThreadsPanel,
  runs: RunsPanel,
  qa: QaTab,
  // Restores layouts from earlier builds, whose Tasks, Files and Design panels are then closed.
  tasks: () => null,
  files: () => null,
  design: () => null,
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
      };
      commands.saveLayout(workspace, JSON.stringify(layout));
    }, 500);
  }

  async function readyMain({ api }: DockviewReadyEvent) {
    latest.current.main = api;
    const saved = await load();
    if (latest.current.main !== api) return;
    restoreLayout(api, saved.main, () => defaultLayout(api));
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
    // A new workspace opens the panel on a terminal, and an open panel never comes up empty.
    const fresh = !saved.main && !saved.bottom;
    if (api.panels.length === 0 && (fresh || (shown.current && bottom.get().open))) {
      if (fresh && shown.current) flushSync(() => bottom.set({ open: true }));
      addTerminal(api, undefined, setup);
    }
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
