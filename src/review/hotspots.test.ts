import { expect, test } from "vitest";
import type { HotspotDto } from "../bindings";
import { hotRows, stepHotspot } from "./hotspots";
import type { DisplayLine } from "./model";

const hot = (id: number, start: number, end: number, level: HotspotDto["level"]): HotspotDto => ({
  id,
  path: "a.rs",
  file: 0,
  rows: { start, end },
  level,
  reason: "r",
  source: "headless",
});

const lines: DisplayLine[] = [
  { kind: "file", file: 0 },
  ...[0, 1, 2, 3, 4].map((r) => ({ kind: "rows" as const, file: 0, hunk: 0, rows: [r] })),
];

test("a row shows its worst hotspot", () => {
  const rows = hotRows([hot(1, 1, 3, "high"), hot(2, 3, 4, "low")]);
  expect(rows.get("0:3")?.id).toBe(1);
  expect(rows.get("0:4")?.id).toBe(2);
  expect(rows.has("0:0")).toBe(false);
});

test("stepping moves between hotspot starts and wrap around", () => {
  const spots = [hot(1, 1, 1, "high"), hot(2, 3, 4, "low")];
  expect(stepHotspot(lines, spots, 0, 1)).toBe(2);
  expect(stepHotspot(lines, spots, 2, 1)).toBe(4);
  expect(stepHotspot(lines, spots, 4, 1)).toBe(2);
  expect(stepHotspot(lines, spots, 2, -1)).toBe(4);
  expect(stepHotspot(lines, [], 0, 1)).toBeUndefined();
});
