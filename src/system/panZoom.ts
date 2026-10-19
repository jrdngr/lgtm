import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

/** The world's offset in the viewport, and its scale. */
export interface View {
  x: number;
  y: number;
  k: number;
}

export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 2;
/** What the zoom buttons multiply by. */
const STEP = 1.25;
/** Around the fitted world. */
const MARGIN = 24;
/** A drag shorter than this is a click. */
const SLOP = 3;

const clamp = (k: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, k));

/** Scales `v` by `factor`, keeping the world point under viewport point (`px`, `py`) still. */
export function zoomAt(v: View, factor: number, px: number, py: number): View {
  const k = clamp(v.k * factor);
  const r = k / v.k;
  return { k, x: px - (px - v.x) * r, y: py - (py - v.y) * r };
}

/** The view that shows the whole `w` by `h` world centered in a `vw` by `vh` viewport, no
 * larger than its own size. */
export function fit(w: number, h: number, vw: number, vh: number): View {
  const k = clamp(Math.min(1, (vw - MARGIN * 2) / w, (vh - MARGIN * 2) / h));
  return { k, x: (vw - w * k) / 2, y: (vh - h * k) / 2 };
}

/** Pans by dragging the background and zooms with the wheel or a pinch, toward the pointer.
 * Fits the world whenever its size changes. `dragged()` says whether the last pointer press
 * panned, so its click can be ignored. */
export function usePanZoom(width: number, height: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<View>({ x: 0, y: 0, k: 1 });
  const moved = useRef(false);

  const fitNow = useCallback(() => {
    const el = ref.current;
    if (el) setView(fit(width, height, el.clientWidth, el.clientHeight));
  }, [width, height]);
  useLayoutEffect(fitNow, [fitNow]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      // A pinch arrives as a wheel with Control held, in much smaller steps.
      const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.002));
      setView((v) => zoomAt(v, factor, e.clientX - r.left, e.clientY - r.top));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const start = { x: e.clientX, y: e.clientY };
    let last = start;
    moved.current = false;
    const move = (ev: PointerEvent) => {
      if (!moved.current && Math.hypot(ev.clientX - start.x, ev.clientY - start.y) < SLOP) return;
      moved.current = true;
      const dx = ev.clientX - last.x;
      const dy = ev.clientY - last.y;
      last = { x: ev.clientX, y: ev.clientY };
      setView((v) => ({ ...v, x: v.x + dx, y: v.y + dy }));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const zoomBy = (factor: number) => {
    const el = ref.current;
    if (el) setView((v) => zoomAt(v, factor, el.clientWidth / 2, el.clientHeight / 2));
  };

  return {
    ref,
    view,
    onPointerDown,
    dragged: () => moved.current,
    zoomIn: () => zoomBy(STEP),
    zoomOut: () => zoomBy(1 / STEP),
    fit: fitNow,
  };
}
