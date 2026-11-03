/** The window's scale (`devicePixelRatio`), which zooming and moving to another screen change.
 * The saved zoom lands right after load by way of 1.0, so a change is reported once the ratio
 * has held still, even when it came back to where it was: anything rasterized for the passing
 * ratio is stale either way. */

/** How long the ratio holds still before a change counts. */
const SETTLE_MS = 100;

const listeners = new Set<() => void>();
let ratio = 0;
let moved = false;
let settling: number | undefined;
let query: MediaQueryList | undefined;

function settle() {
  clearTimeout(settling);
  settling = window.setTimeout(() => {
    watch();
    if (devicePixelRatio === ratio && !moved) return;
    ratio = devicePixelRatio;
    moved = false;
    for (const f of listeners) f();
  }, SETTLE_MS);
}

function changed() {
  moved = true;
  settle();
}

function watch() {
  query?.removeEventListener("change", changed);
  query = matchMedia(`(resolution: ${devicePixelRatio}dppx)`);
  query.addEventListener("change", changed);
}

function start() {
  ratio = devicePixelRatio;
  watch();
  // A zoom also resizes the window in CSS pixels.
  window.addEventListener("resize", settle);
}

function stop() {
  query?.removeEventListener("change", changed);
  query = undefined;
  window.removeEventListener("resize", settle);
  clearTimeout(settling);
}

/** Reports that the window moved to a screen with another backing scale. WebKit doesn't always
 * pass that on as a ratio change while the page is zoomed, which leaves xterm drawing glyphs
 * for one ratio into cells sized for the other. */
export function screenChanged() {
  if (listeners.size > 0) changed();
}

/** Calls `f` once the window's scale has changed and settled. */
export function onScaleSettled(f: () => void): () => void {
  if (listeners.size === 0) start();
  listeners.add(f);
  return () => {
    listeners.delete(f);
    if (listeners.size === 0) stop();
  };
}
