import type { CSSProperties } from "react";
import type { Decor } from "./decor";
import { HEAT_MS } from "./heat";

/** VS Code's decoration colors, as RGB channels, for a glow's tint. */
const TINT: Record<Decor, string> = {
  conflict: "228 103 107",
  added: "129 184 139",
  renamed: "115 201 145",
  modified: "226 192 141",
};

/** A row's glow after a write at `at`, fading out over `HEAT_MS`. Keyed by the write, so a
 * new one starts the fade over. */
export function HeatGlow({ at, decor }: { at?: number; decor?: Decor }) {
  if (at === undefined || Date.now() - at >= HEAT_MS) return null;
  const style = {
    "--heat": TINT[decor ?? "modified"],
    animationDelay: `${at - Date.now()}ms`,
  } as CSSProperties;
  return <span key={at} className="heat-glow" style={style} />;
}
