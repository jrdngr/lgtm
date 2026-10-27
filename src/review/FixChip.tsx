import type { FixDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openMenu } from "../ui/contextMenu";
import { fixTime, openFix } from "./fixes";

interface Props {
  workspace: WorkspaceName;
  /** The thread's fixes, oldest first. */
  fixes: FixDto[];
  /** The fix the review shows, in the Fix view. */
  shown?: number;
}

/** A thread's linked fixes: the latest one's line counts. Clicking it shows that fix; its menu
 * lists every one. */
export function FixChip({ workspace, fixes, shown }: Props) {
  const latest = fixes[fixes.length - 1];
  const on = fixes.some((f) => f.id === shown);
  function menu(e: React.MouseEvent) {
    openMenu(
      e,
      [...fixes].reverse().map((f) => ({
        label: `${fixTime(f)}  +${f.added} −${f.removed}`,
        onSelect: () => openFix(workspace, f.id),
      })),
    );
  }
  return (
    <button
      type="button"
      className={`fix-chip${on ? " on" : ""}`}
      onClick={(e) => {
        e.stopPropagation();
        openFix(workspace, latest.id);
      }}
      onContextMenu={menu}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M8 2.5v6M5 5.5h6M5 13h6" />
      </svg>
      <span>Fix</span>
      {fixes.length > 1 && <span className="fix-n">{fixes.length}</span>}
      <span className="fix-add">+{latest.added}</span>
      <span className="fix-del">−{latest.removed}</span>
    </button>
  );
}
