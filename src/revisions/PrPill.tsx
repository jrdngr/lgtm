import { dockOf } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";
import { openPrPreview } from "../github/preview";
import { openUrl } from "../services/pathMenu";
import { copy, openMenu } from "../ui/contextMenu";
import type { RowPr } from "./useRowPrs";

interface Props {
  workspace: WorkspaceName;
  pr: RowPr;
  /** The workspace's PR. */
  on: boolean;
  /** Kept as the workspace's PR whatever the review. */
  pinned: boolean;
  onReview(): void;
  /** Pins or unpins it. None for the PR the workspace was checked out for. */
  onPin?(pinned: boolean): void;
}

/** An open PR on its head commit. Clicking a stack PR pins it; clicking the workspace's own PR
 * reviews it. */
export function PrPill({ workspace, pr, on, pinned, onReview, onPin }: Props) {
  function menu(e: React.MouseEvent) {
    e.stopPropagation();
    const api = dockOf(workspace);
    openMenu(e, [
      { label: "Review PR", onSelect: onReview },
      ...(onPin ? [{ label: pinned ? "Unpin PR" : "Pin PR", onSelect: () => onPin(!pinned) }] : []),
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
      onClick={onPin ? () => onPin(!pinned) : onReview}
      onContextMenu={menu}
    >
      {pinned && (
        <svg
          className="lr-pin"
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-label="Pinned"
        >
          <path d="M12 17v5M9 3h6l-1 6 3 4H7l3-4z" />
        </svg>
      )}
      #{pr.number}
    </button>
  );
}
