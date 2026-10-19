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
import { CommentsPanel } from "../threads/CommentsPanel";
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

/** The right side panel: the repo's files; the review's changes, comments, architecture and
 * reports; and Q&A. A tab turns amber for threads that need you, broken intents and open high
 * Look items, and open Q&A questions. */
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
        { id: "repo", group: "repo", title: "Files", icon: "folder", render: () => <RepoTab /> },
        {
          id: "changes",
          group: "review",
          title: "Changes",
          icon: "gitTree",
          render: () => <FilesPanel />,
        },
        {
          id: "comments",
          group: "review",
          title: "Comments",
          icon: "comments",
          alert: needsYou(review?.threads ?? []) > 0,
          render: () => <CommentsPanel />,
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
          group: "qa",
          title: "Q&A",
          icon: "help",
          alert: qa.sessions.some((d) => !d.archived && openQuestions(d).length > 0),
          render: () => <QaPanel />,
        },
      ]}
    />
  );
}
