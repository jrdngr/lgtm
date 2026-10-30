import { useEffect, useSyncExternalStore } from "react";
import type { ArchViewDto } from "../bindings";
import { commands, unwrap } from "../ipc";

/** What the map shows until the repo's stored view loads. */
const DEFAULT_VIEW: ArchViewDto = {
  untracked: false,
  tests: false,
  generated: false,
  small: true,
  outside: true,
  groups: true,
  labels: "on",
  unaffected: "hide",
};

let view = DEFAULT_VIEW;
/** Counts the views stored, so the map loads again after each. */
let saved = 0;
/** Whether the stored view was asked for yet. */
let asked = false;
const listeners = new Set<() => void>();

const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

function put(next: ArchViewDto) {
  view = next;
  for (const f of listeners) f();
}

/** The repo's map view, shared by everything in the window, and a setter that stores it. */
export function useArchView(): [ArchViewDto, (change: Partial<ArchViewDto>) => void] {
  const current = useSyncExternalStore(subscribe, () => view);
  useEffect(() => {
    if (asked) return;
    asked = true;
    unwrap(commands.archView()).then(put, () => {});
  }, []);
  return [current, setArchView];
}

/** Choices the backend applies to the map itself. */
const MAP_KEYS: (keyof ArchViewDto)[] = [
  "untracked",
  "tests",
  "generated",
  "small",
  "outside",
  "groups",
];

function setArchView(change: Partial<ArchViewDto>) {
  const last = view;
  put({ ...view, ...change });
  unwrap(commands.setArchView(view)).then(
    () => {
      if (!MAP_KEYS.some((k) => k in change)) return;
      saved++;
      put(view);
    },
    () => put(last),
  );
}

/** Changes each time a view is stored, for loading the map again. */
export const useViewSaves = () => useSyncExternalStore(subscribe, () => saved);
