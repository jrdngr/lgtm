import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import type { Fonts } from "../bindings";
import { watchSettings } from "../settings/store";

/** Each font setting's text and code families, set as `--sans` and `--mono` on the root. */
const FAMILIES: Record<Fonts, { sans: string; mono: string }> = {
  system: {
    sans: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    mono: "Menlo, ui-monospace, monospace",
  },
  geist: {
    sans: '"Geist Variable", -apple-system, sans-serif',
    mono: '"Geist Mono Variable", Menlo, monospace',
  },
};

/** The code family for `fonts`, for what can't read CSS variables, like the terminal. */
export const monoFamily = (fonts: Fonts) => FAMILIES[fonts].mono;

function apply(fonts: Fonts) {
  const root = document.documentElement.style;
  root.setProperty("--sans", FAMILIES[fonts].sans);
  root.setProperty("--mono", FAMILIES[fonts].mono);
}

/** Sets the system fonts now, then follows the font setting. */
export function installFonts() {
  apply("system");
  watchSettings((s) => apply(s.fonts));
}
