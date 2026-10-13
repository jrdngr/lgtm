import type { ChangeDto } from "../bindings";
import type { WorkspaceName } from "../workspaces/context";
import { workspaceStore } from "./workspaceStore";

/** What a workspace's active review pane reviews, for the revision picker in the header.
 * Published even when the review didn't load, so the picker can always choose another. */
export interface ReviewTarget {
  revset: string;
  /** The review's changes, oldest first. Empty until it loads. */
  changes: ChangeDto[];
  loading: boolean;
  /** The working copy is stale, so edits aren't picked up. */
  stale?: boolean;
  /** Loads a revset as the review. Returns the error message when it fails. */
  load(revset: string): Promise<string | undefined>;
  /** Gives keys back to the review pane. */
  focus(): void;
}

const store = workspaceStore<ReviewTarget>();
const openers = new Set<(workspace: WorkspaceName) => void>();

export const publishTarget = store.publish;
/** Clears the workspace's target only if `owner` is still it. */
export const retractTarget = store.retract;
export const targetOf = store.get;
export const isTarget = store.isOwner;
export const useReviewTarget = store.use;
export const onTargetChange = store.subscribe;

/** Asks the header to open the revision picker, as `r` in a review does. */
export function openRevisions(workspace: WorkspaceName) {
  for (const o of openers) o(workspace);
}

export function onOpenRevisions(f: (workspace: WorkspaceName) => void) {
  openers.add(f);
  return () => {
    openers.delete(f);
  };
}
