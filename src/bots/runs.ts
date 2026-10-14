import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useMemo, useSyncExternalStore } from "react";
import { type BotRun, events } from "../bindings";
import { commands } from "../ipc";
import { type ActivityState, report } from "../workspaces/activity";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** Persona review runs the window has seen, kept current by the backend's events. */
const runs = new Map<number, BotRun>();
/** Runs the backend no longer has, so a pane can tell them from runs still loading. */
const gone = new Set<number>();
const listeners = new Set<() => void>();
/** The latest fetch per run, so an older snapshot that resolves late is dropped. */
const latest = new Map<number, number>();
let version = 0;

function changed() {
  version++;
  for (const l of listeners) l();
}

/** How a run or fix stands, from its parts' statuses. */
function stateOf(active: boolean, statuses: ActivityState[]): ActivityState {
  if (active) return "running";
  if (statuses.includes("failed")) return "failed";
  return statuses.includes("stopped") ? "stopped" : "done";
}

/** Tells the workspace's activity how the review and each of its fixes stand. */
function reportRun(run: BotRun) {
  const parts = run.parts.map((p) => p.status);
  report(run.workspace, `bot:${run.id}`, stateOf(!!run.active, parts), "Reviewers");
  for (const [i, f] of (run.fixes ?? []).entries()) {
    report(run.workspace, `bot:${run.id}:fix:${i}`, f.status, "Fix findings");
  }
}

function put(run: BotRun) {
  runs.set(run.id, run);
  gone.delete(run.id);
  reportRun(run);
  changed();
}

export async function refreshRun(id: number) {
  const seq = (latest.get(id) ?? 0) + 1;
  latest.set(id, seq);
  const r = await commands.botRun(id);
  if (latest.get(id) !== seq) return;
  if (r.status === "ok") put(r.data);
  else forgetRun(id);
}

/** Stores a run the window hasn't seen yet, such as one just started. */
export function seedRun(run: BotRun) {
  if (!runs.has(run.id)) put(run);
}

/** Runs a command on `run`, reports its error or that there's none, and refreshes the run. */
export async function runAction(
  run: BotRun,
  f: () => Promise<{ status: string; error?: string }>,
  onError: (message: string | undefined) => void,
) {
  const r = await f();
  onError(r.status === "error" ? r.error : undefined);
  await refreshRun(run.id);
}

export async function loadRuns(workspace: WorkspaceName) {
  const r = await commands.botRuns(workspace);
  if (r.status !== "ok") return;
  for (const run of r.data) {
    runs.set(run.id, run);
    reportRun(run);
  }
  changed();
}

/** Shows what a running part is doing now. */
function setActivity(id: number, part: number, activity: string) {
  const run = runs.get(id);
  if (run?.parts[part]?.status !== "running") return;
  const parts = run.parts.map((x, i) => (i === part ? { ...x, activity } : x));
  put({ ...run, parts });
}

/** Shows a running fix's reply so far. */
function setFixText(id: number, fix: number, text: string) {
  const run = runs.get(id);
  if (run?.fixes?.[fix]?.status !== "running") return;
  const fixes = (run.fixes ?? []).map((f, i) => (i === fix ? { ...f, text } : f));
  put({ ...run, fixes });
}

export function forgetRun(id: number) {
  runs.delete(id);
  gone.add(id);
  changed();
}

let listening = false;

function listen() {
  if (listening) return;
  listening = true;
  const win = getCurrentWebviewWindow();
  events.botRunChanged(win).listen((e) => refreshRun(e.payload.run));
  events
    .botRunActivity(win)
    .listen((e) => setActivity(e.payload.run, e.payload.part, e.payload.activity));
  events.botFixText(win).listen((e) => setFixText(e.payload.run, e.payload.fix, e.payload.text));
}

function subscribe(l: () => void) {
  listen();
  listeners.add(l);
  return () => listeners.delete(l);
}

const useVersion = () => useSyncExternalStore(subscribe, () => version);

/** The workspace's runs, newest first. */
export function useRuns(workspace: WorkspaceName): BotRun[] {
  const v = useVersion();
  useEffect(() => {
    loadRuns(workspace);
  }, [workspace]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `v` says when the map changed
  return useMemo(
    () =>
      [...runs.values()]
        .filter((r) => workspaceKey(r.workspace) === workspaceKey(workspace))
        .sort((a, b) => b.id - a.id),
    [v, workspace],
  );
}

/** The run, `undefined` while it loads, or `null` once the backend doesn't have it. */
export function useRun(id: number): BotRun | null | undefined {
  useVersion();
  useEffect(() => {
    if (!runs.has(id)) refreshRun(id);
  }, [id]);
  return runs.get(id) ?? (gone.has(id) ? null : undefined);
}
