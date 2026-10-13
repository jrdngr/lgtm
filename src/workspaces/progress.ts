import type { ProgressDto } from "../bindings";

/** The share of a workspace's changed rows looked at, kept under 1 until none are left so the
 * ring shows ✓ only when the review is done. */
export function doneShare(p: ProgressDto): number {
  if (p.unreviewed === 0) return 1;
  return Math.min((p.changed - p.unreviewed) / p.changed, 0.99);
}

/** The counts behind the share, for a tooltip. */
export function progressTitle(p: ProgressDto): string {
  return `${p.reviewed} reviewed, ${p.skipped} skipped, ${p.flagged} flagged, ${p.unreviewed} left of ${p.changed}\nProgress of ${p.revset}`;
}
