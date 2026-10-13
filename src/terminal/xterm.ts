import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { watchSettings } from "../settings/store";

/** Opens an xterm in `container`, on WebGL when the webview supports it. */
export function createTerminal(container: HTMLElement) {
  const term = new Terminal({
    fontFamily: "Menlo, monospace",
    fontSize: 13,
    scrollback: 10_000,
    macOptionIsMeta: true,
    theme: { background: "#1e1e1e" },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(container);

  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    term.loadAddon(webgl);
  } catch {
    // The DOM renderer is the fallback.
  }

  /** Resizes to the container. Skipped while the container is hidden. */
  const refit = () => {
    if (container.offsetWidth > 0 && container.offsetHeight > 0) fit.fit();
  };
  refit();
  // Follows the font size setting until the terminal is disposed.
  const unwatch = watchSettings((s) => {
    if (term.options.fontSize === s.terminalFontSize) return;
    term.options.fontSize = s.terminalFontSize;
    refit();
  });
  return { term, refit, unwatch };
}
