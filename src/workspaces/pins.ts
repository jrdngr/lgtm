import { useSyncExternalStore } from "react";
import { loadState, saveState } from "../core/savedState";

/** Colors a workspace row can have, in menu order. */
export const COLORS = {
  blue: "#58a6ff",
  green: "#3fb950",
  orange: "#f0883e",
  yellow: "#e3b341",
  red: "#f85149",
  pink: "#db61a2",
  violet: "#a371f7",
  cyan: "#39c5cf",
} as const;

export type PinColor = keyof typeof COLORS;

/** Pinned workspaces in order, and row colors, by workspace key. Saved per repo. */
export interface Pins {
  pinned: string[];
  colors: Record<string, PinColor>;
}

const EMPTY: Pins = { pinned: [], colors: {} };
const NAME = "workspace-pins";

const valid = (v: unknown): v is Pins => {
  const p = v as Pins | null;
  return (
    !!p &&
    Array.isArray(p.pinned) &&
    p.pinned.every((k) => typeof k === "string") &&
    typeof p.colors === "object" &&
    p.colors !== null &&
    Object.values(p.colors).every((c) => c in COLORS)
  );
};

let pins: Pins | undefined;
const listeners = new Set<() => void>();

function current(): Pins {
  pins ??= loadState(NAME, EMPTY, valid);
  return pins;
}

function set(next: Pins) {
  pins = next;
  saveState(NAME, next);
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export const usePins = () => useSyncExternalStore(subscribe, current);

export function togglePin(key: string) {
  const { pinned, colors } = current();
  set({
    colors,
    pinned: pinned.includes(key) ? pinned.filter((k) => k !== key) : [...pinned, key],
  });
}

export function setColor(key: string, color: PinColor | null) {
  const { pinned, colors } = current();
  const next = { ...colors };
  if (color) next[key] = color;
  else delete next[key];
  set({ pinned, colors: next });
}

/** Moves pinned `key` to `index` among the pins. */
export function movePin(key: string, index: number) {
  const { pinned, colors } = current();
  set({ colors, pinned: moved(pinned, key, index) });
}

/** Drops a deleted workspace's pin and color. */
export function forgetPin(key: string) {
  const { pinned, colors } = current();
  if (!pinned.includes(key) && !(key in colors)) return;
  const next = { ...colors };
  delete next[key];
  set({ pinned: pinned.filter((k) => k !== key), colors: next });
}

/** `list` with `key` moved to `index`, counted in the list without it. */
export function moved(list: string[], key: string, index: number): string[] {
  const rest = list.filter((k) => k !== key);
  rest.splice(Math.max(0, Math.min(index, rest.length)), 0, key);
  return rest;
}

/** Splits `items` into pinned ones, in pin order, and the rest, in their own order. */
export function arrange<T>(items: T[], key: (item: T) => string, pinned: string[]) {
  const byKey = new Map(items.map((i) => [key(i), i]));
  const top = pinned.flatMap((k) => byKey.get(k) ?? []);
  const rest = items.filter((i) => !pinned.includes(key(i)));
  return { top, rest };
}
