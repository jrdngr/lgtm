import { useMemo } from "react";
import type { MarkColor, MarkSlot } from "../bindings";
import { useSettings } from "../settings/store";

/** Every mark color, in palette order. */
const MARK_COLORS: MarkColor[] = [
  "green",
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
  /** Lines can be marked. Without marks, a file is reviewed only by checking it. */
  enabled: boolean;
  slots: MarkSlot[];
  /** The colors in use, in key order: the first is on 1. */
  shown: MarkSlot[];
  done: DoneFn;
  label(color: MarkColor): string;
  /** The first color in use, when it counts as done: the mark that checks a whole file. */
  primary: MarkColor | undefined;
}

export function markSetup(slots: MarkSlot[], enabled = true): MarkSetup {
  const by = new Map(slots.map((s) => [s.color, s]));
  const shown = enabled ? slots.filter((s) => s.shown) : [];
  return {
    enabled,
    slots,
    shown,
    done: (c) => by.get(c)?.done ?? true,
    label: (c) => by.get(c)?.label.trim() || c,
    primary: shown[0]?.done ? shown[0].color : undefined,
  };
}

export function useMarkSetup(): MarkSetup {
  const settings = useSettings();
  const slots = settings?.marks ?? DEFAULT_SLOTS;
  const enabled = settings?.reviewMarks ?? false;
  return useMemo(() => markSetup(slots, enabled), [slots, enabled]);
}

/** The color shown at number key `key` ("1".."9"), if one is in use there. */
export function colorForKey(setup: MarkSetup, key: string): MarkColor | undefined {
  const n = Number(key);
  return Number.isInteger(n) && n >= 1 ? setup.shown[n - 1]?.color : undefined;
}

/** The CSS color of a mark. */
export const markVar = (c: MarkColor) => `var(--mark-${c})`;
