import type { Category } from "../bindings";
import { Icon } from "../ui/icons";
import { CATEGORIES } from "./notes";

/** A note category's colored line icon. An important note takes its level's color. */
export function NoteIcon({ category, level }: { category: Category; level?: string | null }) {
  return (
    <span className={`note-icon note-icon-${category}${level ? ` level-${level}` : ""}`}>
      <Icon name={CATEGORIES[category].icon} size={14} />
    </span>
  );
}

const FILLED: Record<string, number> = { high: 3, medium: 2, low: 1 };

/** An important note's level as three dots, filled one for low up to all three for high. */
export function LevelDots({ level }: { level: string }) {
  const filled = FILLED[level] ?? 0;
  return (
    <span className={`level-dots level-${level}`} role="img" aria-label={level}>
      {[0, 1, 2].map((i) => (
        <span key={i} className={i < filled ? "on" : undefined} />
      ))}
    </span>
  );
}
