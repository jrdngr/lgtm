import type { ProgressDto } from "../bindings";
import type { MarkSetup } from "../review/marks";

/** The share of a workspace's changed rows that are done, kept under 1 until none are left so
 * the ring shows ✓ only when the review is done. */
export function doneShare(p: ProgressDto): number {
  if (p.done === p.changed) return 1;
  return Math.min(p.done / p.changed, 0.99);
}

/** The counts behind the share, for a tooltip. */
export function progressTitle(p: ProgressDto, setup: MarkSetup): string {
  const marks = p.marks.map((m) => `${m.rows} ${setup.label(m.color)}`);
  const counts = [...marks, `${p.changed - p.done} left of ${p.changed}`].join(", ");
  return `${counts}\nProgress of ${p.revset}`;
}
