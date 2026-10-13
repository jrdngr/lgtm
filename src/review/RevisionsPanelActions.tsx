import { useState } from "react";

interface Props {
  onReview(): void;
  /** Runs the key's action on the cursor commit: `e`, `n`, `N` or `s`. */
  onKey(key: string): void;
}

/** Mouse buttons on the cursor row, each doing what its key does. They keep focus on the
 * panel so its keys keep working after a click. */
export function RevisionsPanelActions({ onReview, onKey }: Props) {
  const [newOpen, setNewOpen] = useState(false);
  const button = (label: string, title: string, run: () => void) => (
    <button
      type="button"
      title={title}
      onMouseDown={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
      onDoubleClick={(e) => e.stopPropagation()}
      onClick={() => {
        setNewOpen(false);
        run();
      }}
    >
      {label}
    </button>
  );
  return (
    <span className="log-actions">
      {button("Review", "Review it (↵)", onReview)}
      {button("Edit", "Edit it (e)", () => onKey("e"))}
      <span className="log-new">
        {button("New ▾", "New commit after or before it", () => setNewOpen(!newOpen))}
        {newOpen && (
          <span className="menu-list">
            {button("New after (n)", "New commit after it (n)", () => onKey("n"))}
            {button("New before (N)", "New commit before it (N)", () => onKey("N"))}
          </span>
        )}
      </span>
      {button("Squash", "Squash it into its parent (s)", () => onKey("s"))}
    </span>
  );
}
