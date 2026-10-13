import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { useRuns } from "../bots/runs";
import { RepoTree } from "../editor/RepoTree";
import { SymbolsPanel } from "../graph/SymbolsPanel";
import { CommentsPanel } from "../review/CommentsPanel";
import { useCurrentReview } from "../review/current";
import { FilesPanel } from "../review/FilesPanel";
import { WalkthroughPanel } from "../review/WalkthroughPanel";
import { useWorkspace, workspaceKey } from "../workspaces/context";
import { SidePanel, type SideState } from "./SidePanel";

function RepoTab() {
  return <RepoTree key={workspaceKey(useWorkspace())} />;
}

interface Props {
  state: SideState;
  onChange(next: SideState): void;
  bodyRef: React.RefObject<HTMLDivElement | null>;
  onOpenReport(run: BotRun): void;
}

/** The right side panel: the review's changes and symbols, the repo's files, comments, the
 * walkthrough, and reviewers. Tabs count pending GitHub posts, notes, and running reviews. */
export function RightPanel({ onOpenReport, ...props }: Props) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const runs = useRuns(workspace);
  const pending = review?.comments
    .flatMap((c) => c.github?.posts ?? [])
    .filter((p) => p.pending).length;
  return (
    <SidePanel
      side="right"
      {...props}
      views={[
        { id: "repo", title: "Files", icon: "folder", render: () => <RepoTab /> },
        { id: "changes", title: "Changes", icon: "gitTree", render: () => <FilesPanel /> },
        { id: "symbols", title: "Symbols", icon: "symbols", render: () => <SymbolsPanel /> },
        {
          id: "comments",
          title: "Comments",
          icon: "comments",
          badge: pending,
          badgeTone: "amber",
          render: () => <CommentsPanel />,
        },
        {
          id: "walkthrough",
          title: "Walkthrough",
          icon: "map",
          badge: review?.notes.length,
          render: () => <WalkthroughPanel />,
        },
        {
          id: "reviewers",
          title: "Reviewers",
          icon: "users",
          badge: runs.filter((r) => r.active).length,
          render: () => <BotsPanel onOpen={onOpenReport} />,
        },
      ]}
    />
  );
}
