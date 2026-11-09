import { useSyncExternalStore } from "react";
import { remembered } from "../core/remembered";
import { loadState, saveState } from "../core/savedState";

const HIDE_REVIEWED = "changes-hide-reviewed";
let hideReviewed = loadState(HIDE_REVIEWED, true, (v): v is boolean => typeof v === "boolean");
const listeners = new Set<() => void>();
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** Whether reviewed files go in their own collapsed list in Changes. */
export const useHideReviewed = () => useSyncExternalStore(subscribe, () => hideReviewed);

export function toggleHideReviewed() {
  hideReviewed = !hideReviewed;
  saveState(HIDE_REVIEWED, hideReviewed);
  for (const l of listeners) l();
}

/** Whether the review leaves out PR comments. */
export const hidePrComments = remembered("lgtm:hidePrComments", false, [true, false]);
