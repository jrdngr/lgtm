import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { onPinch } from "../editor/pinch";

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

/** The view that fills a `vw` by `vh` viewport with the whole world, from (`left`, `top`) to
 * (`w`, `h`), centered. */
export function fitView(w: number, h: number, vw: number, vh: number, left = 0, top = 0): View {
  const ww = w - left;
  const wh = h - top;
  const k = clamp(Math.min((vw - MARGIN * 2) / ww, (vh - MARGIN * 2) / wh));
  return { k, x: (vw - ww * k) / 2 - left * k, y: (vh - wh * k) / 2 - top * k };
}

/** Pans by dragging the background or with two fingers, and zooms with a pinch or ⌘ and the
 * wheel, toward the pointer.
 * Fits the world whenever its size changes, or with `fitKey`, whenever that does, and again as
 * the viewport resizes until the view is panned or zoomed. `dragged()` says whether the last
 * pointer press panned, so its click can be ignored. */
export function usePanZoom(width: number, height: number, fitKey?: string, left = 0, top = 0) {
  const ref = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<View>({ x: 0, y: 0, k: 1 });
  const moved = useRef(false);
  /** Whether the view is still the fitted one. */
  const fitted = useRef(true);

  const fitNow = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    fitted.current = true;
    setView(fitView(width, height, el.clientWidth, el.clientHeight, left, top));
  }, [width, height, left, top]);
  const fitWhen = fitKey ?? `${width}x${height}`;
  // biome-ignore lint/correctness/useExhaustiveDependencies: refits only when `fitWhen` changes
  useLayoutEffect(fitNow, [fitWhen]);

  // A tab laid out hidden has no size yet; it fits once it has one.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      if (fitted.current && el.clientWidth && el.clientHeight) fitNow();
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [fitNow]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const zoom = (factor: number, x: number, y: number) => {
      const r = el.getBoundingClientRect();
      fitted.current = false;
      setView((v) => zoomAt(v, factor, x - r.left, y - r.top));
    };
    // Two fingers pan; a pinch, or the wheel with ⌘ held, zooms.
    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey) return;
      e.preventDefault();
      if (e.metaKey) return zoom(Math.exp(-e.deltaY * 0.002), e.clientX, e.clientY);
      const line = e.deltaMode === 1 ? 16 : 1;
      fitted.current = false;
      setView((v) => ({ ...v, x: v.x - e.deltaX * line, y: v.y - e.deltaY * line }));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    const stopPinch = onPinch(el, zoom);
    return () => {
      el.removeEventListener("wheel", onWheel);
      stopPinch();
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    // A press on a node clicks it, whatever the last press did.
    moved.current = false;
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const start = { x: e.clientX, y: e.clientY };
    let last = start;
    const move = (ev: PointerEvent) => {
      if (!moved.current && Math.hypot(ev.clientX - start.x, ev.clientY - start.y) < SLOP) return;
      moved.current = true;
      fitted.current = false;
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
    if (!el) return;
    fitted.current = false;
    setView((v) => zoomAt(v, factor, el.clientWidth / 2, el.clientHeight / 2));
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
