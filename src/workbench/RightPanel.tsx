import { ArchSection } from "../arch/sidebar/ArchSection";
import { archBadge } from "../arch/sidebar/logic";
import { useArch } from "../arch/state";
import type { BotRun } from "../bindings";
import { BotsPanel } from "../bots/BotsPanel";
import { useWorkspace, workspaceKey } from "../core/workspace";
import { RepoTree } from "../editor/RepoTree";
import { GuidedPanel } from "../guided/GuidedPanel";
import { needsYou } from "../model/stages";
import { openQuestions } from "../qa/model";
import { QaPanel } from "../qa/QaPanel";
import { useQa } from "../qa/store";
import { useCurrentReview } from "../review/current";
import { FilesPanel } from "../review/FilesPanel";
import { SearchPanel } from "../search/SearchPanel";
import { CommentsPanel } from "../threads/CommentsPanel";
import { ArtifactsPanel } from "./ArtifactsPanel";
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

/** The right side panel: the repo's files, search, and the review's changes and comments; the
 * guided review, architecture, reports and Q&A; and notes and drawings. A tab turns amber for threads that need you, open high Look items,
 * and open Q&A questions. */
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
        { id: "repo", group: "code", title: "Files", icon: "folder", render: () => <RepoTab /> },
        {
          id: "search",
          group: "code",
          title: "Search",
          icon: "search",
          render: () => <SearchPanel />,
        },
        {
          id: "changes",
          group: "code",
          title: "Changes",
          icon: "gitTree",
          render: () => <FilesPanel />,
        },
        {
          id: "comments",
          group: "code",
          title: "Comments",
          icon: "comments",
          alert: needsYou(review?.threads ?? []) > 0,
          render: () => <CommentsPanel />,
        },
        {
          id: "guided",
          group: "review",
          title: "Guided",
          icon: "guided",
          render: () => <GuidedPanel />,
        },
        {
          id: "architecture",
          group: "review",
          title: "Architecture",
          icon: "architecture",
          alert: archBadge(arch) > 0,
          render: () => <ArchSection />,
        },
        {
          id: "reviewers",
          group: "review",
          title: "Reports",
          icon: "users",
          render: () => <BotsPanel onOpen={onOpenReport} />,
        },
        {
          id: "qa",
          group: "review",
          title: "Q&A",
          icon: "help",
          alert: qa.sessions.some((d) => !d.archived && openQuestions(d).length > 0),
          render: () => <QaPanel />,
        },
        {
          id: "artifacts",
          group: "artifacts",
          title: "Artifacts",
          icon: "note",
          render: () => <ArtifactsPanel />,
        },
      ]}
    />
  );
}
