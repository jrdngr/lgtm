import { useMemo } from "react";
import type { MarkColor, MarkSlot } from "../bindings";
import { useSettings } from "../settings/store";

/** Every mark color, in palette order. */
export const MARK_COLORS: MarkColor[] = [
  "cyan",
  "red",
  "gray",
  "orange",
  "violet",
  "pink",
  "yellow",
  "blue",
  "tan",
];

/** The setup before settings load, as the backend's defaults. */
export const DEFAULT_SLOTS: MarkSlot[] = MARK_COLORS.map((color, i) => ({
  color,
  label: ["reviewed", "flagged", "skipped"][i] ?? "",
  done: color !== "red",
  shown: i < 3,
}));

/** Whether rows marked `color` count as done. */
export type DoneFn = (color: MarkColor) => boolean;

/** What each color means, from the settings. */
export interface MarkSetup {
  slots: MarkSlot[];
  /** The colors in use, in key order: the first is on 1. */
  shown: MarkSlot[];
  done: DoneFn;
  label(color: MarkColor): string;
  /** The first color in use, when it counts as done: what a checked file's rows show as,
   * and the mark that checks a whole file. */
  primary: MarkColor | undefined;
}

export function markSetup(slots: MarkSlot[]): MarkSetup {
  const by = new Map(slots.map((s) => [s.color, s]));
  const shown = slots.filter((s) => s.shown);
  return {
    slots,
    shown,
    done: (c) => by.get(c)?.done ?? true,
    label: (c) => by.get(c)?.label.trim() || c,
    primary: shown[0]?.done ? shown[0].color : undefined,
  };
}

export function useMarkSetup(): MarkSetup {
  const slots = useSettings()?.marks ?? DEFAULT_SLOTS;
  return useMemo(() => markSetup(slots), [slots]);
}

/** The color shown at number key `key` ("1".."9"), if one is in use there. */
export function colorForKey(setup: MarkSetup, key: string): MarkColor | undefined {
  const n = Number(key);
  return Number.isInteger(n) && n >= 1 ? setup.shown[n - 1]?.color : undefined;
}

/** The CSS color of a mark. */
export const markVar = (c: MarkColor) => `var(--mark-${c})`;
