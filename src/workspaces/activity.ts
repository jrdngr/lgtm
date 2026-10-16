import { useSyncExternalStore } from "react";
import { type WorkspaceName, workspaceKey } from "./context";

/** What an agent reports: `running`, waiting on `input`, ended `done`, `failed` or `stopped`,
 * at its prompt (`idle`), or `gone`. */
export type ActivityState = "running" | "input" | "done" | "failed" | "stopped" | "idle" | "gone";

/** How a workspace's row shows its agents, most urgent first. */
export const AGENT_STATES = ["input", "failed", "stalled", "working", "done", "idle"] as const;
export type AgentState = (typeof AGENT_STATES)[number];

/** The workspace's most urgent agent state, how many agents it has, and the labels of the
 * agents in that state. */
export interface Activity {
  state: AgentState | "none";
  count: number;
  labels: string[];
}

/** A running agent with no output for this long counts as stalled. */
export const STALL_MS = 3 * 60_000;
const TICK_MS = 15_000;

interface Agent {
  key: string;
  label: string;
  state: "working" | "input" | "idle" | "done" | "failed";
  /** When it last showed signs of life. */
  pulse: number;
  /** Stays as idle once its ending is seen, like Claude in a terminal. Others go. */
  stays: boolean;
}

const agents = new Map<string, Agent>();
const cache = new Map<string, Activity>();
const listeners = new Set<() => void>();
let selected = "";
let ticking: ReturnType<typeof setInterval> | undefined;

function changed() {
  cache.clear();
  for (const l of listeners) l();
  // Stalls appear with time, not with reports.
  const working = [...agents.values()].some((a) => a.state === "working");
  if (working && ticking === undefined) ticking = setInterval(changed, TICK_MS);
  if (!working && ticking !== undefined) {
    clearInterval(ticking);
    ticking = undefined;
  }
}

/** Reports agent `id` in `workspace`. An ending in the selected workspace counts as seen. An
 * agent that `stays` goes idle once seen; others go. Endings of an agent that isn't working
 * or waiting are ignored, so repeated reports don't bring one back. */
export function report(
  workspace: WorkspaceName,
  id: string,
  state: ActivityState,
  label: string,
  stays = false,
) {
  const key = workspaceKey(workspace);
  const a = agents.get(id);
  const set = (s: Agent["state"]) => {
    if (a?.key === key && a.state === s && a.label === label) return;
    agents.set(id, { key, label, state: s, pulse: Date.now(), stays });
    changed();
  };
  switch (state) {
    case "running":
      return set("working");
    case "input":
      return set("input");
    case "idle":
      return set("idle");
    case "done":
    case "failed":
      if (a?.state !== "working" && a?.state !== "input") return;
      if (key !== selected) return set(state);
      return stays ? set("idle") : gone(id);
    case "stopped":
      return stays ? set("idle") : gone(id);
    case "gone":
      return gone(id);
  }
}

function gone(id: string) {
  if (agents.delete(id)) changed();
}

/** Agent `id` showed signs of life, so it isn't stalled. */
export function pulse(id: string) {
  const a = agents.get(id);
  if (!a) return;
  const stalled = isStalled(a, Date.now());
  a.pulse = Date.now();
  if (stalled) changed();
}

const isStalled = (a: Agent, now: number) => a.state === "working" && now - a.pulse > STALL_MS;

/** Marks `workspace` as the one shown, which counts what ended in it as seen. */
export function selectActivity(workspace: WorkspaceName) {
  selected = workspaceKey(workspace);
  clearFinished(workspace);
}

/** Counts what ended in `workspace` as seen, without selecting it. */
export function clearFinished(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  let any = false;
  for (const [id, a] of agents) {
    if (a.key !== key || (a.state !== "done" && a.state !== "failed")) continue;
    any = true;
    if (a.stays) a.state = "idle";
    else agents.delete(id);
  }
  if (any) changed();
}

export function activity(workspace: WorkspaceName): Activity {
  const key = workspaceKey(workspace);
  const hit = cache.get(key);
  if (hit) return hit;
  const now = Date.now();
  const mine = [...agents.values()]
    .filter((a) => a.key === key)
    .map((a) => ({ label: a.label, state: isStalled(a, now) ? ("stalled" as const) : a.state }));
  const state = AGENT_STATES.find((s) => mine.some((a) => a.state === s)) ?? "none";
  const labels = mine.filter((a) => a.state === state).map((a) => a.label);
  const result: Activity = { state, count: mine.length, labels };
  cache.set(key, result);
  return result;
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useActivity(workspace: WorkspaceName): Activity {
  return useSyncExternalStore(subscribe, () => activity(workspace));
}
