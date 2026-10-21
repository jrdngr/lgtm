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

/** Drops the workspace's stack PR once the review is anything but the PR's commits. */
export async function prFollowsReview(workspace: WorkspaceName, revset: string) {
  const pr = await prOf(workspace);
  if (!pr?.stack || pr.revset === revset) return;
  const r = await commands.clearStackPr(workspace);
  if (r.status === "ok") setPr(workspace, null);
}
