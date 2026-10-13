import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { RepoTree } from "../editor/RepoTree";
import { SymbolsPanel } from "../graph/SymbolsPanel";
import { CommentsPanel } from "../review/CommentsPanel";
import { FilesPanel } from "../review/FilesPanel";
import { WalkthroughPanel } from "../review/WalkthroughPanel";
import { useWorkspace, workspaceKey } from "../workspaces/context";
import { Segmented } from "./Segmented";
import { SidePanel, type SideState } from "./SidePanel";
import { diffView, reportView } from "./subviews";

function DiffTab() {
  const view = diffView.use();
  return (
    <div className="side-split">
      <Segmented
        options={[
          ["files", "Files"],
          ["symbols", "Symbols"],
        ]}
        value={view}
        onChange={diffView.set}
      />
      <div className="side-split-body">{view === "files" ? <FilesPanel /> : <SymbolsPanel />}</div>
    </div>
  );
}

function ReportsTab({ onOpen }: { onOpen(run: BotRun): void }) {
  const view = reportView.use();
  return (
    <div className="side-split">
      <Segmented
        options={[
          ["walkthrough", "Walkthrough"],
          ["reviewers", "Reviewers"],
        ]}
        value={view}
        onChange={reportView.set}
      />
      <div className="side-split-body">
        {view === "walkthrough" ? <WalkthroughPanel /> : <BotsPanel onOpen={onOpen} />}
      </div>
    </div>
  );
}

function FilesTab() {
  return <RepoTree key={workspaceKey(useWorkspace())} />;
}

interface Props {
  state: SideState;
  onChange(next: SideState): void;
  bodyRef: React.RefObject<HTMLDivElement | null>;
  onOpenReport(run: BotRun): void;
}

/** The right side panel: the review's diff, the repo's files, comments, and reports. */
export function RightPanel({ onOpenReport, ...props }: Props) {
  return (
    <SidePanel
      side="right"
      {...props}
      views={[
        { id: "diff", title: "Diff", icon: "diff", render: () => <DiffTab /> },
        { id: "repo", title: "Files", icon: "folder", render: () => <FilesTab /> },
        { id: "comments", title: "Comments", icon: "comments", render: () => <CommentsPanel /> },
        {
          id: "reports",
          title: "Reports",
          icon: "map",
          render: () => <ReportsTab onOpen={onOpenReport} />,
        },
      ]}
    />
  );
}
