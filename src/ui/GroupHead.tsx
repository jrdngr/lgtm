import type { ReactNode } from "react";

interface Props {
  /** Left out for a list's only group. */
  name?: ReactNode;
  count: ReactNode;
  /** A stage's color for its dot; "" for grey. No dot when undefined. */
  tone?: string;
  /** Folded. Given with `onToggle`, the head folds and unfolds its group. */
  shut?: boolean;
  onToggle?(): void;
  onContextMenu?(e: React.MouseEvent): void;
}

/** The head of a group in a side panel list: a chevron when it folds, a stage dot, its name and
 * a count. */
export function GroupHead({ name, count, tone, shut, onToggle, onContextMenu }: Props) {
  const inner = (
    <>
      {onToggle && <span className="chevron">{shut ? "▸" : "▾"}</span>}
      {tone !== undefined && <span className={`dot stage-seg ${tone || "grey"}`} />}
      {name !== undefined && <span>{name}</span>}
      <span className="group-count">{count}</span>
    </>
  );
  return onToggle ? (
    <button
      type="button"
      className="group-head fold"
      aria-expanded={!shut}
      onClick={onToggle}
      onContextMenu={onContextMenu}
    >
      {inner}
    </button>
  ) : (
    <div className="group-head" onContextMenu={onContextMenu}>
      {inner}
    </div>
  );
}
