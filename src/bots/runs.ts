import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useMemo, useSyncExternalStore } from "react";
import { type BotRun, events } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands } from "../ipc";

/** Reports the window has seen, kept current by the backend's events. */
const runs = new Map<number, BotRun>();
/** Reports the backend no longer has, so a pane can tell them from runs still loading. */
const gone = new Set<number>();
const listeners = new Set<() => void>();
/** The latest fetch per run, so an older snapshot that resolves late is dropped. */
const latest = new Map<number, number>();
let version = 0;

function changed() {
  version++;
  for (const l of listeners) l();
}

function put(run: BotRun) {
  runs.set(run.id, run);
  gone.delete(run.id);
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

async function loadRuns(workspace: WorkspaceName) {
  const r = await commands.botRuns(workspace);
  if (r.status !== "ok") return;
  for (const run of r.data) runs.set(run.id, run);
  changed();
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
  events.botRunChanged(getCurrentWebviewWindow()).listen((e) => refreshRun(e.payload.run));
}

function subscribe(l: () => void) {
  listen();
  listeners.add(l);
  return () => listeners.delete(l);
}

const useVersion = () => useSyncExternalStore(subscribe, () => version);

/** The workspace's reports, newest first. */
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

/** The report, `undefined` while it loads, or `null` once the backend doesn't have it. */
export function useRun(id: number): BotRun | null | undefined {
  useVersion();
  useEffect(() => {
    if (!runs.has(id)) refreshRun(id);
  }, [id]);
  return runs.get(id) ?? (gone.has(id) ? null : undefined);
}
