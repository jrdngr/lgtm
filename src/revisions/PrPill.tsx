import { dockOf } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";
import { openPrPreview } from "../github/preview";
import { openUrl } from "../services/pathMenu";
import { copy, openMenu } from "../ui/contextMenu";
import type { RowPr } from "./useRowPrs";

interface Props {
  workspace: WorkspaceName;
  pr: RowPr;
  /** The review is this PR's commits. */
  on: boolean;
  onReview(): void;
}

/** An open PR on its head commit. Clicking it reviews the PR. */
export function PrPill({ workspace, pr, on, onReview }: Props) {
  function menu(e: React.MouseEvent) {
    e.stopPropagation();
    const api = dockOf(workspace);
    openMenu(e, [
      { label: "Review PR", onSelect: onReview },
      { label: "Preview", disabled: !api, onSelect: () => api && openPrPreview(api, pr) },
      "-",
      { label: "Open on GitHub", onSelect: () => openUrl(pr.url) },
      { label: "Copy PR URL", onSelect: () => copy(pr.url) },
      { label: "Copy Bookmark Name", onSelect: () => copy(pr.bookmark) },
    ]);
  }
  return (
    <button
      type="button"
      className={`pill pr lr-pr${on ? " on" : ""}`}
      title={pr.title}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onClick={onReview}
      onContextMenu={menu}
    >
      #{pr.number}
    </button>
  );
}
