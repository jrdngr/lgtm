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

const LEVEL_PILL: Record<string, string> = { high: "pill red", medium: "pill amber" };

/** An important note's level as a pill: High, Medium, or Low. */
export function LevelPill({ level }: { level: string }) {
  return (
    <span className={LEVEL_PILL[level] ?? "pill"}>
      {level.charAt(0).toUpperCase() + level.slice(1)}
    </span>
  );
}
