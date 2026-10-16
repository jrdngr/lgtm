import { useSyncExternalStore } from "react";
import { type WorkspaceName, workspaceKey } from "./context";

/** What an agent reports: `running`, waiting on `input`, ended `done`, `failed` or `stopped`,
 * at its prompt (`idle`), or `gone`. */
export type ActivityState = "running" | "input" | "done" | "failed" | "stopped" | "idle" | "gone";

/** How a workspace's row shows its agents, most urgent first. */
export const AGENT_STATES = ["input", "failed", "stalled", "working", "done", "idle"] as const;
export type AgentState = (typeof AGENT_STATES)[number];

/** One of a workspace's agents as its row lists it. */
export interface AgentInfo {
  id: string;
  label: string;
  state: AgentState;
  /** When it took that state. */
  since: number;
  /** A headless run rather than an agent in a terminal. */
  headless: boolean;
  /** The panel it runs in, or `REVIEW_TAB` for any review tab. */
  tab?: string;
}

/** The workspace's most urgent agent state and its agents, most urgent first. */
export interface Activity {
  state: AgentState | "none";
  agents: AgentInfo[];
}

/** An agent's `tab` that stands for every review tab. */
export const REVIEW_TAB = "review";

export interface ReportOptions {
  /** Stays as idle once its ending is seen, like Claude in a terminal. Others go. */
  stays?: boolean;
  headless?: boolean;
  tab?: string;
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
  /** When it took its state. */
  since: number;
  stays: boolean;
  headless: boolean;
  tab?: string;
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
  { stays = false, headless = false, tab }: ReportOptions = {},
) {
  const key = workspaceKey(workspace);
  const a = agents.get(id);
  const set = (s: Agent["state"]) => {
    if (a?.key === key && a.state === s && a.label === label && a.tab === tab) return;
    const now = Date.now();
    const since = a?.state === s ? a.since : now;
    agents.set(id, { key, label, state: s, pulse: now, since, stays, headless, tab });
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
    if (a.stays) {
      a.state = "idle";
      a.since = Date.now();
    } else agents.delete(id);
  }
  if (any) changed();
}

export function activity(workspace: WorkspaceName): Activity {
  const key = workspaceKey(workspace);
  const hit = cache.get(key);
  if (hit) return hit;
  const now = Date.now();
  const mine = [...agents]
    .filter(([, a]) => a.key === key)
    .map(([id, a]) => info(id, a, now))
    .sort((a, b) => rank(a.state) - rank(b.state) || b.since - a.since);
  const result: Activity = { state: mine[0]?.state ?? "none", agents: mine };
  cache.set(key, result);
  return result;
}

const rank = (s: AgentState) => AGENT_STATES.indexOf(s);

function info(id: string, a: Agent, now: number): AgentInfo {
  const stalled = isStalled(a, now);
  return {
    id,
    label: a.label,
    state: stalled ? "stalled" : a.state,
    since: stalled ? a.pulse + STALL_MS : a.since,
    headless: a.headless,
    tab: a.tab,
  };
}

/** The most urgent agent running in panel `id`, in any workspace. */
export function tabAgent(id: string): AgentInfo | undefined {
  const now = Date.now();
  return [...agents]
    .filter(([, a]) => a.tab === id)
    .map(([key, a]) => info(key, a, now))
    .sort((a, b) => rank(a.state) - rank(b.state))[0];
}

/** The most urgent state of the agents running in panel `id`, a review tab when `review`. */
export function tabState(a: Activity, id: string, review: boolean): AgentState | undefined {
  return a.agents.find((g) => g.tab === id || (review && g.tab === REVIEW_TAB))?.state;
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useActivity(workspace: WorkspaceName): Activity {
  return useSyncExternalStore(subscribe, () => activity(workspace));
}
