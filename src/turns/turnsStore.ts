import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events, type TurnDto } from "../bindings";
import { remembered } from "../core/remembered";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands } from "../ipc";
import { workspaceStore } from "../review/workspaceStore";
import { onRepoChanged } from "../workspaces/useRepoChanged";

/** Each workspace's agent turns and checkpoints on `@`, newest first. */
const store = workspaceStore<TurnDto[]>();
const watched = new Set<string>();
const NONE: TurnDto[] = [];

/** Whether the turns under `@` in the revisions log are shown. */
export const turnsOpen = remembered("turnsOpen", false, [true, false]);

export async function reloadTurns(workspace: WorkspaceName) {
  const r = await commands.agentTurns(workspace);
  if (r.status === "ok") store.publish(workspace, r.data);
}

/** Starts keeping the workspace's turns current: on repo changes and on agents' hooks. */
function watch(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  if (watched.has(key)) return;
  watched.add(key);
  reloadTurns(workspace);
  onRepoChanged(() => reloadTurns(workspace), workspace);
  events.turnsChanged(getCurrentWebviewWindow()).listen((e) => {
    if (workspaceKey(e.payload.workspace) === key) reloadTurns(workspace);
  });
}

export function useTurns(workspace: WorkspaceName): TurnDto[] {
  useEffect(() => watch(workspace), [workspace]);
  return store.use(workspace) ?? NONE;
}

/** "3 turns, 2 edits, 1 checkpoint". */
export function turnsLabel(turns: TurnDto[]): string {
  const n = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;
  const count = (kind: TurnDto["kind"]) => turns.filter((t) => t.kind === kind).length;
  const parts: [number, string][] = [
    [count("agent"), "turn"],
    [count("edit"), "edit"],
    [count("checkpoint"), "checkpoint"],
  ];
  return parts
    .filter(([c]) => c)
    .map(([c, one]) => n(c, one))
    .join(", ");
}

/** A turn's name: its prompt's first line, the names of the files saved, or "Checkpoint". */
export function turnName(t: TurnDto): string {
  if (t.kind === "edit") return t.paths.map((p) => p.split("/").pop()).join(", ");
  return t.prompt?.split("\n")[0] ?? "Checkpoint";
}

/** A turn's start time, as `14:02`. */
export function turnTime(t: TurnDto): string {
  return new Date(t.at ?? 0).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
