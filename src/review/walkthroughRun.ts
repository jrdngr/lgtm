import { Channel } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { startRun } from "../agents/runs";
import type { StateDto } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";

/** A walkthrough run on one review. */
export interface WalkthroughRun {
  running: boolean;
  /** The agent's latest tool call while it runs, or how the last run ended. */
  note?: string;
  error?: string;
}

const runs = new Map<number, WalkthroughRun>();
const listeners = new Set<() => void>();

function set(review: number, run: WalkthroughRun) {
  runs.set(review, run);
  for (const l of listeners) l();
}

/** Starts an agent writing the walkthrough of `review`, which shows `revset`. `onState` gets
 * the review's state with the new notes. */
export async function runWalkthrough(
  workspace: WorkspaceName,
  review: number,
  revset: string,
  onState: (s: StateDto) => void,
) {
  if (runs.get(review)?.running) return;
  set(review, { running: true, note: "Starting…" });
  const handle = startRun(workspace, {
    kind: "walkthrough",
    subject: revset,
    stop: () => stopWalkthrough(review),
    retry: () => runWalkthrough(workspace, review, revset, onState),
  });
  const activity = new Channel<string>();
  activity.onmessage = (note) => {
    set(review, { running: true, note });
    handle.activity(note);
  };
  const r = await commands.runWalkthrough(review, activity);
  if (r.status === "error") {
    handle.end("failed", r.error);
    return set(review, { running: false, error: r.error });
  }
  if (!r.data) {
    handle.end("stopped");
    return set(review, { running: false, note: "Stopped" });
  }
  onState(r.data.state);
  const n = r.data.state.notes.length;
  const skipped = r.data.skipped ? `, ${r.data.skipped} outside the review left out` : "";
  const note = `${n} ${n === 1 ? "note" : "notes"}${skipped}`;
  handle.end("done", note);
  set(review, { running: false, note });
}

export function stopWalkthrough(review: number) {
  commands.stopWalkthrough(review);
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useWalkthroughRun(review: number | undefined): WalkthroughRun | undefined {
  return useSyncExternalStore(subscribe, () =>
    review === undefined ? undefined : runs.get(review),
  );
}
