/** WebKit's trackpad pinch, which isn't in the DOM types. `scale` runs from 1 at the start. */
interface GestureEvent extends UIEvent {
  scale: number;
  clientX: number;
  clientY: number;
}

/** How much a ctrl+wheel (a pinch, outside WebKit) zooms, from its deltaY. */
export function wheelFactor(e: Pick<WheelEvent, "deltaY" | "deltaMode">): number {
  const px = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * 400 : e.deltaY;
  return Math.exp(-Math.max(-50, Math.min(50, px)) * 0.01);
}

/** Calls `zoom` with each step of a trackpad pinch over `el`, as a factor and the point it's
 * around, and stops the webview zooming itself. Returns a function that stops listening. */
export function onPinch(
  el: HTMLElement,
  zoom: (factor: number, x: number, y: number) => void,
): () => void {
  let last = 0;
  const start = (e: Event) => {
    e.preventDefault();
    last = 1;
  };
  const change = (e: Event) => {
    e.preventDefault();
    const g = e as GestureEvent;
    if (!last || g.scale <= 0) return;
    zoom(g.scale / last, g.clientX, g.clientY);
    last = g.scale;
  };
  const end = (e: Event) => {
    e.preventDefault();
    last = 0;
  };
  const wheel = (e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    if (!last) zoom(wheelFactor(e), e.clientX, e.clientY);
  };
  el.addEventListener("gesturestart", start);
  el.addEventListener("gesturechange", change);
  el.addEventListener("gestureend", end);
  el.addEventListener("wheel", wheel, { passive: false });
  return () => {
    el.removeEventListener("gesturestart", start);
    el.removeEventListener("gesturechange", change);
    el.removeEventListener("gestureend", end);
    el.removeEventListener("wheel", wheel);
  };
}
