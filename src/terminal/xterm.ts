import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { watchSettings } from "../settings/store";

/** How long a terminal stays hidden before it gives up its WebGL context. */
const PARK_MS = 30_000;

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
  // Shift+Enter sends Esc+Enter, the newline key in Claude Code, as its /terminal-setup does
  // for VS Code. xterm would send a plain Enter.
  term.attachCustomKeyEventHandler((e) => {
    const shiftEnter = e.key === "Enter" && e.shiftKey && !e.altKey && !e.ctrlKey && !e.metaKey;
    if (!shiftEnter) return true;
    if (e.type === "keydown") term.input("\x1b\r");
    return false;
  });

  /** Its WebGL renderer, while it has one. WebKit allows only a few WebGL contexts at once, so
   * a terminal hidden for a while hands its context back and draws with the DOM renderer. */
  let webgl: WebglAddon | undefined;
  const attach = () => {
    if (webgl) return;
    try {
      const w = new WebglAddon();
      w.onContextLoss(() => {
        w.dispose();
        if (webgl === w) webgl = undefined;
      });
      term.loadAddon(w);
      webgl = w;
    } catch {
      // The DOM renderer is the fallback.
    }
  };
  attach();
  let parking: number | undefined;
  const shown = new IntersectionObserver(([e]) => {
    clearTimeout(parking);
    if (e?.isIntersecting) attach();
    else
      parking = window.setTimeout(() => {
        webgl?.dispose();
        webgl = undefined;
      }, PARK_MS);
  });
  shown.observe(container);

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
  /** Stops following settings and visibility, before the terminal is disposed. */
  const release = () => {
    unwatch();
    shown.disconnect();
    clearTimeout(parking);
  };
  return { term, refit, release };
}
