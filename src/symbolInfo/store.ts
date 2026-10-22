import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState, useSyncExternalStore } from "react";
import { events, type SymbolInfoDto } from "../bindings";
import { loadState, saveState } from "../core/savedState";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";

export const infoKey = (i: { path: string; name: string }) => `${i.path}\0${i.name}`;

let version = 0;
let listening = false;
const listeners = new Set<() => void>();
const notify = () => {
  for (const l of listeners) l();
};

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Listens once for Symbol Information changes, from the app or an agent. */
function listen() {
  if (listening) return;
  listening = true;
  events.symbolInfoChanged(getCurrentWebviewWindow()).listen(() => {
    version++;
    notify();
  });
}

/** Symbol Information on symbols defined in `paths`, loaded again after any change. */
export function useSymbolInfos(ws: WorkspaceName, paths: string[]): SymbolInfoDto[] {
  listen();
  const v = useSyncExternalStore(subscribe, () => version);
  const [infos, setInfos] = useState<SymbolInfoDto[]>([]);
  const key = paths.join("\n");
  // biome-ignore lint/correctness/useExhaustiveDependencies: `v` asks for a reload
  useEffect(() => {
    if (!key) return setInfos([]);
    let live = true;
    unwrap(commands.symbolInfos(ws, key.split("\n"))).then(
      (i) => live && setInfos(i),
      () => live && setInfos([]),
    );
    return () => {
      live = false;
    };
  }, [ws, key, v]);
  return infos;
}

const strings = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((s) => typeof s === "string");

let folded: ReadonlySet<string> | undefined;

function foldedSet(): ReadonlySet<string> {
  folded ??= new Set(loadState("symbolInfoFolds", [], strings));
  return folded;
}

/** Symbol Information folded to its gutter icon, by `infoKey`, remembered. */
export function useFolded(): ReadonlySet<string> {
  return useSyncExternalStore(subscribe, foldedSet);
}

export function setFolded(key: string, fold: boolean) {
  const next = new Set(foldedSet());
  if (fold) next.add(key);
  else next.delete(key);
  folded = next;
  saveState("symbolInfoFolds", [...next]);
  notify();
}

export const dismiss = (i: SymbolInfoDto) => commands.dismissSymbolInfo(i.path, i.name);
