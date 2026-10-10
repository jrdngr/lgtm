import { useSyncExternalStore } from "react";
import type { CheckState, CommentDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import type { FlatFile } from "./model";
import type { Filter, Progress } from "./state";

/** What the last active review pane shows, for panels that follow it (files, comments). */
export interface CurrentReview {
  files: FlatFile[];
  progress: Progress[];
  checks: CheckState[];
  filter: Filter;
  comments: CommentDto[];
  /** Scrolls the review pane to `file` and puts the cursor there. */
  jump(file: number): void;
  /** Scrolls the review pane to a comment, or its file when its code is gone. */
  jumpToComment(id: number): void;
  /** Pastes text into one of the workspace's terminals, asking which when there are several. */
  send(text: string): void;
}

/** One current review per workspace, since each has its own panes. */
const current = new Map<string, CurrentReview>();
const listeners = new Set<() => void>();

export function publishReview(workspace: WorkspaceName, next: CurrentReview | undefined) {
  if (next) current.set(workspaceKey(workspace), next);
  else current.delete(workspaceKey(workspace));
  for (const l of listeners) l();
}

/** Clears the workspace's current review only if `owner` is still it. */
export function retractReview(workspace: WorkspaceName, owner: CurrentReview | undefined) {
  if (owner && current.get(workspaceKey(workspace)) === owner) publishReview(workspace, undefined);
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useCurrentReview(workspace: WorkspaceName) {
  return useSyncExternalStore(subscribe, () => current.get(workspaceKey(workspace)));
}
