import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useSyncExternalStore } from "react";
import { events, type GuidedDto } from "../bindings";
import { dockOf } from "../core/docks";
import { openGuidedTab } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands } from "../ipc";

interface Loaded {
  /** `null` when the review has none yet. */
  g?: GuidedDto | null;
  error?: string;
}

/** Each review's guided review, shared by the sidebar and the tab. */
const loaded = new Map<number, Loaded>();
const loading = new Set<number>();
const listeners = new Set<() => void>();
const changed = () => {
  for (const l of listeners) l();
};
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};
const NONE: Loaded = {};

async function load(review: number) {
  if (loading.has(review)) return;
  loading.add(review);
  const r = await commands.guided(review).finally(() => loading.delete(review));
  loaded.set(review, r.status === "ok" ? { g: r.data } : { error: r.error });
  changed();
}

/** Shows `g` as review `review`'s guided review, after a command returned it. */
export function setGuided(review: number, g: GuidedDto) {
  loaded.set(review, { g });
  changed();
}

/** The review's guided review, loaded on first use and again when an agent writes one. */
export function useGuided(review: number | undefined): Loaded {
  useEffect(() => {
    if (review !== undefined && !loaded.has(review)) load(review);
  }, [review]);
  return useSyncExternalStore(subscribe, () =>
    review === undefined ? NONE : (loaded.get(review) ?? NONE),
  );
}

/** Loads the guided review again when a terminal agent writes one in `workspace`. */
export function useAgentGuided(workspace: WorkspaceName) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.guidedChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (workspaceKey(p.workspace) !== key) return;
      load(p.review);
      openGuided(workspace);
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace]);
}

/** What the Guided Review tab shows. */
export type Pick =
  | { kind: "chunk"; id: string }
  /** A PR thread on code no chunk holds, by its comment id. */
  | { kind: "commented"; id: number }
  | { kind: "excluded"; index: number }
  | { kind: "context" };

const picks = new Map<string, Pick>();

export function usePick(workspace: WorkspaceName): Pick | undefined {
  return useSyncExternalStore(subscribe, () => picks.get(workspaceKey(workspace)));
}

export function setPick(workspace: WorkspaceName, pick: Pick | undefined) {
  const key = workspaceKey(workspace);
  if (pick) picks.set(key, pick);
  else picks.delete(key);
  changed();
}

/** Shows `pick` in the workspace's Guided Review tab, opening it. */
export function openGuided(workspace: WorkspaceName, pick?: Pick) {
  if (pick) setPick(workspace, pick);
  const api = dockOf(workspace);
  if (api) openGuidedTab(api);
}

export const samePick = (a: Pick | undefined, b: Pick) =>
  !!a &&
  a.kind === b.kind &&
  (a.kind === "context" ||
    (a.kind === "chunk" && b.kind === "chunk" && a.id === b.id) ||
    (a.kind === "commented" && b.kind === "commented" && a.id === b.id) ||
    (a.kind === "excluded" && b.kind === "excluded" && a.index === b.index));
