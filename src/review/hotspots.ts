import type { HotspotDto } from "../bindings";
import { type DisplayLine, markKey } from "./model";

/** The worst hotspot on each row, by `markKey`. Hotspots come worst first. */
export function hotRows(hotspots: HotspotDto[]): Map<string, HotspotDto> {
  const out = new Map<string, HotspotDto>();
  for (const h of hotspots) {
    if (!h.rows) continue;
    for (let r = h.rows.start; r <= h.rows.end; r++) {
      const key = markKey(h.file, r);
      if (!out.has(key)) out.set(key, h);
    }
  }
  return out;
}

/** The line where each shown hotspot starts, in display order. */
function starts(lines: DisplayLine[], hotspots: HotspotDto[]): number[] {
  const firsts = new Set(hotspots.flatMap((h) => (h.rows ? [markKey(h.file, h.rows.start)] : [])));
  const out: number[] = [];
  lines.forEach((l, i) => {
    if (l.kind === "rows" && l.rows.some((r) => firsts.has(markKey(l.file, r)))) out.push(i);
  });
  return out;
}

/** The line of the next (`dir` 1) or previous (-1) hotspot from `cursor`, wrapping around. */
export function stepHotspot(
  lines: DisplayLine[],
  hotspots: HotspotDto[],
  cursor: number,
  dir: 1 | -1,
): number | undefined {
  const at = starts(lines, hotspots);
  if (at.length === 0) return undefined;
  if (dir === 1) return at.find((i) => i > cursor) ?? at[0];
  return [...at].reverse().find((i) => i < cursor) ?? at[at.length - 1];
}
