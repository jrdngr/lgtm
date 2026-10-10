import { useSyncExternalStore } from "react";
import type { CheckState } from "../bindings";
import type { FlatFile } from "./model";
import type { Filter, Progress } from "./state";

/** What the last active review pane shows, for panels that follow it (the file tree). */
export interface CurrentReview {
  files: FlatFile[];
  progress: Progress[];
  checks: CheckState[];
  filter: Filter;
  /** Scrolls the review pane to `file` and puts the cursor there. */
  jump(file: number): void;
}

let current: CurrentReview | undefined;
const listeners = new Set<() => void>();

export function publishReview(next: CurrentReview | undefined) {
  current = next;
  for (const l of listeners) l();
}

/** Clears the current review only if `owner` is still it. */
export function retractReview(owner: CurrentReview | undefined) {
  if (owner && current === owner) publishReview(undefined);
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useCurrentReview() {
  return useSyncExternalStore(subscribe, () => current);
}
