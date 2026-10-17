import { useSyncExternalStore } from "react";
import { events, type Settings } from "../bindings";
import { commands } from "../ipc";

/** The app's settings, loaded once and kept current as any window saves them. */
let current: Settings | undefined;
const listeners = new Set<() => void>();

function set(next: Settings) {
  current = next;
  for (const l of listeners) l();
}

let started = false;
function start() {
  if (started) return;
  started = true;
  commands.getSettings().then(set);
  events.settingsChanged.listen((e) => set(e.payload.settings));
}

function subscribe(l: () => void) {
  start();
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The settings, `undefined` until they've loaded. */
export function useSettings() {
  return useSyncExternalStore(subscribe, () => current);
}

/** The settings, once they've loaded. */
export function loadedSettings(): Promise<Settings> {
  if (current) return Promise.resolve(current);
  return new Promise((done) => {
    const stop = subscribe(() => {
      if (!current) return;
      stop();
      done(current);
    });
  });
}

/** Calls `on` with the settings now (once loaded) and whenever they change. */
export function watchSettings(on: (s: Settings) => void) {
  const fire = () => current && on(current);
  const stop = subscribe(fire);
  fire();
  return stop;
}

/** Saves `patch` over the current settings. Resolves to an error message, if any. */
export async function saveSettings(patch: Partial<Settings>): Promise<string | undefined> {
  if (!current) return "Settings haven't loaded yet";
  const next = { ...current, ...patch };
  const r = await commands.saveSettings(next);
  if (r.status === "error") return r.error;
  set(next);
}
