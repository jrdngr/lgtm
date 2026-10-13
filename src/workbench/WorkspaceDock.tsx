import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  themeDark,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import { useEffect, useRef, useState } from "react";
import { ReportPanel } from "../bots/ReportPanel";
import { EditorPanel } from "../editor/EditorPanel";
import { useAgentShow } from "../editor/useAgentShow";
import { PrPreview } from "../github/PrPreview";
import { GraphPanel } from "../graph/GraphPanel";
import { commands, unwrap } from "../ipc";
import { WorkspaceReview } from "../review/IdleReview";
import { ReviewPanel } from "../review/ReviewPanel";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { WorkspaceContext, type WorkspaceName } from "../workspaces/context";
import { registerDock } from "./docks";
import { EmptyGroup } from "./EmptyGroup";
import { NewTab, SplitButtons } from "./GroupActions";
import { defaultLayout, dropStalePanels, isReviewPanel } from "./panels";
import { Tab } from "./Tab";

const components = {
  review: ReviewPanel,
  terminal: TerminalPanel,
  report: ReportPanel,
  graph: GraphPanel,
  editor: EditorPanel,
  prPreview: PrPreview,
  // Restores layouts from earlier builds, whose Tasks and Files panels are then closed.
  tasks: () => null,
  files: () => null,
};

function restoreLayout(api: DockviewApi, saved: string | null, setup: boolean) {
  if (saved) {
    try {
      api.fromJSON(JSON.parse(saved));
      dropStalePanels(api);
      if (api.panels.length > 0) return;
    } catch (e) {
      console.warn("discarding unreadable layout", e);
      api.clear();
    }
  }
  defaultLayout(api, setup);
}

interface Props {
  workspace: WorkspaceName;
  hidden: boolean;
  /** A new workspace: its first terminal runs `.lgtm/setup`. */
  setup: boolean;
}

/** One workspace's panes, with its own saved layout. Hidden docks stay mounted so their
 * terminals keep running. */
export function WorkspaceDock({ workspace, hidden, setup }: Props) {
  const timer = useRef<number>(undefined);
  const gone = useRef(false);
  /** The dock's latest api. React can mount the dock twice, and panes added to the first,
   * already disposed api would stay mounted outside the dock. */
  const latest = useRef<DockviewApi>(undefined);
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
    };
  }, [workspace]);

  async function ready({ api }: DockviewReadyEvent) {
    latest.current = api;
    const saved = await unwrap(commands.loadLayout(workspace)).catch(() => null);
    if (latest.current !== api) return;
    restoreLayout(api, saved, setup);
    const panes = () => setHasPane(api.panels.some(isReviewPanel));
    panes();
    api.onDidAddPanel(panes);
    api.onDidRemovePanel(panes);
    api.onDidLayoutChange(() => {
      clearTimeout(timer.current);
      timer.current = window.setTimeout(
        () => gone.current || commands.saveLayout(workspace, JSON.stringify(api.toJSON())),
        500,
      );
    });
    registerDock(workspace, api);
  }

  return (
    <WorkspaceContext.Provider value={workspace}>
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
      {hasPane !== undefined && <WorkspaceReview workspace={workspace} hasPane={hasPane} />}
    </WorkspaceContext.Provider>
  );
}
