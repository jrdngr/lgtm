import { type SetStateAction, useCallback, useState } from "react";

/** UI state remembered across restarts, in localStorage under this window's repo. Panel
 * layouts and each tab's own state live in the saved layout instead. */
let repoRoot = "";

/** Set once a window knows its repo, before anything loads state. */
export function setStateRepo(root: string) {
  repoRoot = root;
}

const key = (name: string) => `lgtm:${name}:${repoRoot}`;

export function loadState<T>(name: string, fallback: T, valid: (v: unknown) => v is T): T {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(key(name)) ?? "null");
    return valid(saved) ? saved : fallback;
  } catch {
    return fallback;
  }
}

export function saveState(name: string, value: unknown) {
  try {
    localStorage.setItem(key(name), JSON.stringify(value));
  } catch {
    // Remembering is a convenience.
  }
}

const strings = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((s) => typeof s === "string");

/** A set of strings, such as open folders, remembered under `name`. */
export function useSavedSet(name: string) {
  const [set, setSet] = useState<ReadonlySet<string>>(() => new Set(loadState(name, [], strings)));
  const update = useCallback(
    (action: SetStateAction<ReadonlySet<string>>) =>
      setSet((prev) => {
        const next = typeof action === "function" ? action(prev) : action;
        saveState(name, [...next]);
        return next;
      }),
    [name],
  );
  return [set, update] as const;
}
