import type { ChangeDto, View } from "../bindings";
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
  /** The agent turns the review shows, in the Agent turn view. */
  turns?: number[];
  /** Loads a revset as the review, in `view` or Combined. Returns the error message when it
   * fails. */
  load(revset: string, view?: View): Promise<string | undefined>;
  /** Shows the review in `view`. */
  view(view: View): void;
  /** Gives keys back to the review pane. */
  focus(): void;
}

const store = workspaceStore<ReviewTarget>();

export const publishTarget = store.publish;
/** Clears the workspace's target only if `owner` is still it. */
export const retractTarget = store.retract;
export const targetOf = store.get;
export const isTarget = store.isOwner;
export const useReviewTarget = store.use;
export const onTargetChange = store.subscribe;
