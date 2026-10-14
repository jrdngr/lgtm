import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { useRuns } from "../bots/runs";
import { RepoTree } from "../editor/RepoTree";
import { usePr } from "../github/pr";
import { SymbolsPanel } from "../graph/SymbolsPanel";
import { openQuestions } from "../qa/model";
import { QaPanel } from "../qa/QaPanel";
import { useQa } from "../qa/store";
import { useCurrentReview } from "../review/current";
import { FilesPanel } from "../review/FilesPanel";
import { NotesPanel } from "../review/NotesPanel";
import { WalkthroughPanel } from "../review/WalkthroughPanel";
import { PrPanel } from "../threads/PrPanel";
import { needsYou } from "../threads/stages";
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
  onHide(): void;
}

/** The right side panel: the review's changes and symbols, the repo's files, notes, the PR in a
 * PR workspace, the walkthrough, reviewers, and Q&A. Tabs count threads that need you,
 * walkthrough notes, running reviews, and open Q&A questions. */
export function RightPanel({ onOpenReport, ...props }: Props) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const runs = useRuns(workspace);
  const pr = usePr(workspace);
  const qa = useQa(workspace);
  return (
    <SidePanel
      side="right"
      {...props}
      views={[
        { id: "repo", title: "Files", icon: "folder", render: () => <RepoTab /> },
        { id: "changes", title: "Changes", icon: "gitTree", render: () => <FilesPanel /> },
        { id: "symbols", title: "Symbols", icon: "symbols", render: () => <SymbolsPanel /> },
        { id: "comments", title: "Notes", icon: "comments", render: () => <NotesPanel /> },
        ...(pr
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
          id: "qa",
          title: "Q&A",
          icon: "help",
          badge: qa.sessions.reduce((n, d) => n + (d.archived ? 0 : openQuestions(d).length), 0),
          badgeTone: "amber",
          render: () => <QaPanel />,
        },
      ]}
    />
  );
}
