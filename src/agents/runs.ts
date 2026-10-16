import { useSyncExternalStore } from "react";
import { pulse, report } from "../workspaces/activity";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

export type AgentKind = "triage" | "fix" | "qa" | "assess" | "intentFix";
export type Outcome = "running" | "done" | "failed" | "stopped";

export const KIND_NAMES: Record<AgentKind, string> = {
  triage: "Triage",
  fix: "Fix threads",
  qa: "Q&A",
  assess: "Assess",
  intentFix: "Fix intents",
};

/** A headless agent run the window started, other than a persona review. */
export interface AgentRun {
  id: number;
  kind: AgentKind;
  /** What it's about, like a file and line or a change id. */
  subject: string;
  started: number;
  outcome: Outcome;
  /** What it's doing now, how it ended, or why it failed. */
  detail?: string;
  stop?(): void;
  /** Starts the same run again. */
  retry?(): void;
  /** Shows what it worked on. */
  open?(): void;
}

export interface RunHandle {
  activity(detail: string): void;
  end(outcome: Exclude<Outcome, "running">, detail?: string): void;
}

/** How many finished runs a workspace keeps. */
const KEEP = 50;
const byWorkspace = new Map<string, AgentRun[]>();
const listeners = new Set<() => void>();
const EMPTY: AgentRun[] = [];
let next = 1;

function update(key: string, id: number, f: (r: AgentRun) => AgentRun) {
  const list = byWorkspace.get(key);
  if (!list?.some((r) => r.id === id)) return;
  byWorkspace.set(
    key,
    list.map((r) => (r.id === id ? f(r) : r)),
  );
  for (const l of listeners) l();
}

/** Lists a run in the workspace's Agent runs, newest first. */
export function startRun(
  workspace: WorkspaceName,
  run: Omit<AgentRun, "id" | "started" | "outcome">,
): RunHandle {
  const key = workspaceKey(workspace);
  const id = next++;
  const list = byWorkspace.get(key) ?? [];
  const kept = list.filter((r, i) => r.outcome === "running" || i < KEEP);
  byWorkspace.set(key, [{ ...run, id, started: Date.now(), outcome: "running" }, ...kept]);
  for (const l of listeners) l();
  const label = KIND_NAMES[run.kind];
  report(workspace, `run:${id}`, "running", label);
  return {
    activity: (detail) => {
      pulse(`run:${id}`);
      update(key, id, (r) => (r.outcome === "running" ? { ...r, detail } : r));
    },
    end: (outcome, detail) => {
      update(key, id, (r) => ({ ...r, outcome, detail }));
      report(workspace, `run:${id}`, outcome, label);
    },
  };
}

/** Ends `handle` from a command's result: failed with its error, else done. */
export function endFrom(handle: RunHandle, r: { status: string; error?: string }, done?: string) {
  if (r.status === "error") handle.end("failed", r.error);
  else handle.end("done", done);
}

/** Drops the workspace's runs that `keep` turns down. */
function prune(workspace: WorkspaceName, keep: (r: AgentRun) => boolean) {
  const key = workspaceKey(workspace);
  const list = byWorkspace.get(key);
  if (!list) return;
  byWorkspace.set(key, list.filter(keep));
  for (const l of listeners) l();
}

export const clearFinished = (workspace: WorkspaceName) =>
  prune(workspace, (r) => r.outcome === "running");

/** Takes a finished run off the list. */
export const removeRun = (workspace: WorkspaceName, id: number) =>
  prune(workspace, (r) => r.id !== id || r.outcome === "running");

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The workspace's runs, newest first. */
export const agentRuns = (workspace: WorkspaceName) =>
  byWorkspace.get(workspaceKey(workspace)) ?? EMPTY;

export function useAgentRuns(workspace: WorkspaceName): AgentRun[] {
  return useSyncExternalStore(subscribe, () => agentRuns(workspace));
}
