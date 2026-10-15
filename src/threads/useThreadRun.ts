import { Channel } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { endFrom, startRun } from "../agents/runs";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";
import { working } from "./stages";

/** A triage run over a review's threads: the threads it covers and what the agent is
 * doing now. */
export interface ThreadRun {
  activity: string;
  threads: ReadonlySet<number>;
}

const runs = new Map<number, ThreadRun>();
const listeners = new Set<() => void>();

function set(review: number, run: ThreadRun | undefined) {
  if (run) runs.set(review, run);
  else runs.delete(review);
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The run on `review`, if one is going. */
export function useRunOn(review: number | undefined) {
  return useSyncExternalStore(subscribe, () => (review == null ? undefined : runs.get(review)));
}

/** Whether a triage run covers comment `id` of `review`. */
export function useTriaging(review: number | undefined, id: number) {
  const run = useRunOn(review);
  return !!run?.threads.has(id);
}

/** Starts and stops triage runs over the PR's threads. */
export function useThreadRun(review: CurrentReview | undefined, onError: (e: string) => void) {
  const workspace = useWorkspace();
  const run = useRunOn(review?.id);

  async function start() {
    if (!review || runs.has(review.id)) return;
    const id = review.id;
    const threads = new Set(
      working(review.threads)
        .filter((t) => t.state.stage === "new")
        .map((t) => t.id),
    );
    set(id, { activity: "", threads });
    const handle = startRun(workspace, {
      kind: "triage",
      subject: "PR threads",
      stop: () => commands.stopThreadRun(id),
      retry: () => start(),
    });
    const channel = new Channel<string>();
    channel.onmessage = (activity) => {
      const r = runs.get(id);
      if (r) set(id, { ...r, activity });
      handle.activity(activity);
    };
    const r = await commands.triageThreads(id, channel);
    set(id, undefined);
    endFrom(handle, r);
    if (r.status === "error") onError(r.error);
    else review.apply(r.data);
  }

  return {
    run,
    start,
    stop: () => review && commands.stopThreadRun(review.id),
  };
}
