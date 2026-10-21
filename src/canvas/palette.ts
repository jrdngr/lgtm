import type { Swatch } from "../bindings";

/** The canvas palette, in toolbar order. */
export const SWATCHES: Swatch[] = [
  "red",
  "orange",
  "yellow",
  "green",
  "cyan",
  "blue",
  "purple",
  "pink",
];

const HEX: Record<Swatch, string> = {
  red: "#e5534b",
  orange: "#e0823d",
  yellow: "#d4a72c",
  green: "#57ab5a",
  cyan: "#39c5cf",
  blue: "#539bf5",
  purple: "#b083f0",
  pink: "#c96198",
};

export const swatchHex = (s: Swatch) => HEX[s];

/** Notes and arrows without a color. */
export const INK = "#e6e6e6";
