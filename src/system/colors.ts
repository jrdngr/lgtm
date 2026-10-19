/** Containers' colors, the biggest first. */
export const PALETTE = [
  "#5b9bd5",
  "#e8913f",
  "#a879f0",
  "#3fb8a9",
  "#d4a72c",
  "#e06c75",
  "#8bc34a",
  "#4fc1e9",
];

/** Files outside every source root. */
export const LOOSE = "#8b8d93";

/** `hex` at `alpha`, as rgb(). */
export function tint(hex: string, alpha: number) {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255} / ${Math.round(alpha * 100)}%)`;
}

/** People, externals and the system. */
export const NEUTRAL = "#d8d8d8";
