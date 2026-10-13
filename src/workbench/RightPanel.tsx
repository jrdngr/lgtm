import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { useRuns } from "../bots/runs";
import { DesignPanel } from "../design/DesignPanel";
import { openQuestions } from "../design/model";
import { useDesigns } from "../design/store";
import { RepoTree } from "../editor/RepoTree";
import { usePr } from "../github/pr";
import { SymbolsPanel } from "../graph/SymbolsPanel";
import { useCurrentReview } from "../review/current";
import { FilesPanel } from "../review/FilesPanel";
import { NotesPanel } from "../review/NotesPanel";
import { WalkthroughPanel } from "../review/WalkthroughPanel";
import { PrPanel } from "../threads/PrPanel";
import { needsYou } from "../threads/stages";
import { useGithubOff } from "../tools/agents";
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

/** The right side panel: the review's changes and symbols, the repo's files, notes, the PR in a
 * PR workspace, the walkthrough, reviewers, and designs. Tabs count threads that need you,
 * walkthrough notes, running reviews, and open design questions. */
export function RightPanel({ onOpenReport, ...props }: Props) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const runs = useRuns(workspace);
  const pr = usePr(workspace);
  const githubOff = useGithubOff();
  const designs = useDesigns(workspace);
  return (
    <SidePanel
      side="right"
      {...props}
      views={[
        { id: "repo", title: "Files", icon: "folder", render: () => <RepoTab /> },
        { id: "changes", title: "Changes", icon: "gitTree", render: () => <FilesPanel /> },
        { id: "symbols", title: "Symbols", icon: "symbols", render: () => <SymbolsPanel /> },
        { id: "comments", title: "Notes", icon: "comments", render: () => <NotesPanel /> },
        ...(pr && !githubOff
          ? [
              {
                id: "pr",
                title: "PR",
                icon: "pr" as const,
                badge: needsYou(review?.threads ?? []),
                badgeTone: "amber" as const,
                render: () => <PrPanel />,
              },
            ]
          : []),
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
        {
          id: "design",
          title: "Design",
          icon: "layers",
          badge: designs.designs.reduce((n, d) => n + openQuestions(d).length, 0),
          badgeTone: "amber",
          render: () => <DesignPanel />,
        },
      ]}
    />
  );
}
