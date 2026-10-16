import { Channel } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { startRun } from "../agents/runs";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { reloadArch } from "./state";

/** An Assess or intent Fix run on one review. */
export interface ArchRun {
  running: boolean;
  /** The agent's latest tool call while it runs, or how the last run ended. */
  note?: string;
  error?: string;
}

type Kind = "assess" | "intentFix";

const runs = new Map<string, ArchRun>();
const listeners = new Set<() => void>();
const key = (kind: Kind, review: number) => `${kind}:${review}`;

function set(kind: Kind, review: number, run: ArchRun) {
  runs.set(key(kind, review), run);
  for (const l of listeners) l();
}

/** Runs `call` as a `kind` agent run on `review`, then loads the architecture again. `done`
 * says how a finished run ended. */
async function run<T>(
  workspace: WorkspaceName,
  kind: Kind,
  review: number,
  subject: string,
  call: (
    activity: Channel<string>,
  ) => Promise<{ status: "ok"; data: T | null } | { status: "error"; error: string }>,
  stop: () => void,
  retry: () => void,
  done: (data: T) => string,
) {
  if (runs.get(key(kind, review))?.running) return;
  set(kind, review, { running: true, note: "Starting…" });
  const handle = startRun(workspace, { kind, subject, stop, retry });
  const activity = new Channel<string>();
  activity.onmessage = (note) => {
    set(kind, review, { running: true, note });
    handle.activity(note);
  };
  const r = await call(activity);
  if (r.status === "error") {
    handle.end("failed", r.error);
    return set(kind, review, { running: false, error: r.error });
  }
  if (r.data === null) {
    handle.end("stopped");
    return set(kind, review, { running: false, note: "Stopped" });
  }
  const note = done(r.data);
  handle.end("done", note);
  set(kind, review, { running: false, note });
  reloadArch(review);
}

/** Starts an agent assessing the architecture of `review`, which shows `revset`. */
export function runAssess(workspace: WorkspaceName, review: number, revset: string) {
  return run(
    workspace,
    "assess",
    review,
    revset,
    (activity) => commands.runAssess(review, activity),
    () => stopAssess(review),
    () => runAssess(workspace, review, revset),
    (a) => `${a.score}/5`,
  );
}

export const stopAssess = (review: number) => commands.stopAssess(review);

/** Starts an agent making the code meet intents `ids`. */
export function fixIntents(
  workspace: WorkspaceName,
  review: number,
  revset: string,
  ids: number[],
) {
  return run(
    workspace,
    "intentFix",
    review,
    revset,
    (activity) => commands.fixIntents(review, ids, activity),
    () => stopIntentFix(review),
    () => fixIntents(workspace, review, revset, ids),
    (reply) => reply.split("\n").find((l) => l.trim()) ?? "Done",
  );
}

export const stopIntentFix = (review: number) => commands.stopIntentFix(review);

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useArchRun(kind: Kind, review: number | undefined): ArchRun | undefined {
  return useSyncExternalStore(subscribe, () =>
    review === undefined ? undefined : runs.get(key(kind, review)),
  );
}
