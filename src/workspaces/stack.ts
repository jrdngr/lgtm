import type { WorkspaceName } from "../core/workspace";
import { prOf, setPr } from "../github/pr";
import { commands, unwrap } from "../ipc";
import { loadReview } from "../revisions/loadReview";

/** Reviews stack PR `number`: it becomes the workspace's PR, and the review its commits. */
export async function reviewStackPr(workspace: WorkspaceName, number: number) {
  const pr = await unwrap(commands.reviewStackPr(workspace, number));
  setPr(workspace, pr);
  const failed = await loadReview(workspace, pr.revset);
  if (failed) throw new Error(failed);
}

/** Drops the workspace's stack PR once the review leaves the PR's commits, unless it's pinned. */
export async function prFollowsReview(workspace: WorkspaceName, revset: string) {
  const pr = await prOf(workspace);
  if (!pr?.stack || pr.pinned || pr.revset === revset) return;
  const r = await commands.followReview(workspace, revset);
  if (r.status === "ok" && !r.data) setPr(workspace, null);
}

/** Pins or unpins stack PR `number`, selecting it first. An unpinned PR then follows the
 * review. */
export async function pinStackPr(
  workspace: WorkspaceName,
  number: number,
  pinned: boolean,
  revset: string,
) {
  const pr = await unwrap(commands.pinStackPr(workspace, number, pinned));
  setPr(workspace, pr);
  if (!pinned) await prFollowsReview(workspace, revset);
}
