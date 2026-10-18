import { dockOf } from "../core/docks";
import { activate, isReviewPanel, openReviewTab } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { type CurrentReview, currentReview, onReviewChange } from "./current";

/** What the idle review published, which review panes take over. */
const idle = new WeakSet<object>();

export function markIdle<T extends object>(o: T): T {
  idle.add(o);
  return o;
}

export const isIdle = (o: object | undefined) => !!o && idle.has(o);

/** Gives up on a pane that never loads, so its action doesn't fire much later. */
const WAIT_MS = 60_000;
const waiting = new Map<string, () => void>();

/** Calls `f` with the workspace's review pane once one has loaded, showing the dock's review
 * pane first, or opening one when it has none. A later call replaces a waiting one. */
export function inReviewPane(workspace: WorkspaceName, f: (review: CurrentReview) => void) {
  const api = dockOf(workspace);
  if (!api) return;
  const key = workspaceKey(workspace);
  waiting.get(key)?.();
  const done = () => {
    off();
    clearTimeout(timer);
    waiting.delete(key);
  };
  const off = onReviewChange(() => {
    const review = currentReview(workspace);
    if (!review || isIdle(review)) return;
    done();
    f(review);
  });
  const timer = setTimeout(done, WAIT_MS);
  waiting.set(key, done);
  const pane = api.panels.find(isReviewPanel);
  if (pane) activate(pane);
  else openReviewTab(api);
}
