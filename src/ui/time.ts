const UNITS: [number, string][] = [
  [60, "s"],
  [60, "m"],
  [24, "h"],
  [7, "d"],
  [4.35, "w"],
  [12, "mo"],
];

/** How long ago `iso` was, short: "now", "5m", "3h", "2d", "1y". */
export function ago(iso: string | number, now = Date.now()): string {
  let n = (now - new Date(iso).getTime()) / 1000;
  if (!Number.isFinite(n)) return "";
  if (n < 45) return "now";
  for (const [size, unit] of UNITS) {
    if (n < size) return `${Math.floor(n)}${unit}`;
    n /= size;
  }
  return `${Math.floor(n)}y`;
}

/** The full local date and time, for a tooltip. */
export const fullDate = (iso: string | number) => new Date(iso).toLocaleString();
