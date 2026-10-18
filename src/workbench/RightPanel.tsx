import { ArchSection } from "../arch/sidebar/ArchSection";
import { archBadge } from "../arch/sidebar/logic";
import { useArch } from "../arch/state";
import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { useWorkspace, workspaceKey } from "../core/workspace";
import { RepoTree } from "../editor/RepoTree";
import { needsYou } from "../model/stages";
import { openQuestions } from "../qa/model";
import { QaPanel } from "../qa/QaPanel";
import { useQa } from "../qa/store";
import { useCurrentReview } from "../review/current";
import { FilesPanel } from "../review/FilesPanel";
import { NotesPanel } from "../review/NotesPanel";
import { WalkthroughPanel } from "../review/WalkthroughPanel";
import { PrPanel } from "../threads/PrPanel";
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

/** The right side panel: the review's changes and symbols, the repo's files, notes, the
 * workspace's PR, the walkthrough, the architecture, reports, and Q&A. Tabs count threads
 * that need you, walkthrough notes, broken intents and open high Look items, and open Q&A
 * questions. */
export function RightPanel({ onOpenReport, ...props }: Props) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const qa = useQa(workspace);
  const { arch } = useArch(review);
  return (
    <SidePanel
      side="right"
      {...props}
      views={[
        { id: "repo", title: "Files", icon: "folder", render: () => <RepoTab /> },
        { id: "changes", title: "Changes", icon: "gitTree", render: () => <FilesPanel /> },
        { id: "comments", title: "Notes", icon: "comments", render: () => <NotesPanel /> },
        {
          id: "pr",
          title: "PR",
          icon: "pr",
          badge: needsYou(review?.threads ?? []),
          badgeTone: "amber",
          render: () => <PrPanel />,
        },
        {
          id: "walkthrough",
          title: "Walkthrough",
          icon: "map",
          badge: review?.notes.length,
          render: () => <WalkthroughPanel />,
        },
        {
          id: "architecture",
          title: "Architecture",
          icon: "architecture",
          badge: archBadge(arch),
          badgeTone: "amber",
          render: () => <ArchSection />,
        },
        {
          id: "reviewers",
          title: "Reports",
          icon: "users",
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
