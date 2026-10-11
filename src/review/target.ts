import { useSyncExternalStore } from "react";
import type { ChangeDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** What a workspace's active review pane reviews, for the revision picker in the header.
 * Published even when the review didn't load, so the picker can always choose another. */
export interface ReviewTarget {
  revset: string;
  /** The review's changes, oldest first. Empty until it loads. */
  changes: ChangeDto[];
  loading: boolean;
  /** Loads a revset as the review. Returns the error message when it fails. */
  load(revset: string): Promise<string | undefined>;
  /** Gives keys back to the review pane. */
  focus(): void;
}

const targets = new Map<string, ReviewTarget>();
const listeners = new Set<() => void>();
const openers = new Set<(workspace: WorkspaceName) => void>();

function changed() {
  for (const l of listeners) l();
}

export function publishTarget(workspace: WorkspaceName, target: ReviewTarget) {
  targets.set(workspaceKey(workspace), target);
  changed();
}

/** Clears the workspace's target only if `owner` is still it. */
export function retractTarget(workspace: WorkspaceName, owner: ReviewTarget | undefined) {
  const key = workspaceKey(workspace);
  if (owner && targets.get(key) === owner) {
    targets.delete(key);
    changed();
  }
}

/** Whether `owner` is the workspace's target, or there's none when `owner` is undefined. */
export function isTarget(workspace: WorkspaceName, owner: ReviewTarget | undefined) {
  return targets.get(workspaceKey(workspace)) === owner;
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useReviewTarget(workspace: WorkspaceName) {
  return useSyncExternalStore(subscribe, () => targets.get(workspaceKey(workspace)));
}

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
