import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { fitted, stepZoom } from "./htmlNav";
import { onPinch } from "./pinch";
import { ZoomControls, zoomKey } from "./ZoomControls";

/** How an image is sized: shrunk to fit the tab (never enlarged), scaled to fill the tab,
 * or at a zoom. */
export type Zoom = "shrink" | "fit" | number;

/** An image that shrinks to fit its tab until zoomed or fitted. `controls` go in the tab's
 * bar; `view` shows the image and takes the zoom keys while focused. */
export function useImageZoom(
  src: string | undefined,
  alt: string,
  saved: Zoom = "shrink",
  onZoom?: (zoom: Zoom) => void,
) {
  const [zoom, setZoomState] = useState<Zoom>(saved);
  const setZoom = (z: Zoom) => {
    setZoomState(z);
    onZoom?.(z);
  };
  const [size, setSize] = useState<[number, number]>();
  /** The room the view has for the image. */
  const [room, setRoom] = useState<[number, number]>();
  const box = useRef<HTMLDivElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `src` mounts the view to measure
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => {
      const pad = Number.parseFloat(getComputedStyle(el).paddingLeft) * 2;
      setRoom([el.clientWidth - pad, el.clientHeight - pad]);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [src]);

  /** Unknown until the image loads, or for an SVG with no size of its own. */
  const known = size && size[0] > 0 && size[1] > 0 ? size : undefined;
  const fill = known && room ? Math.min(room[0] / known[0], room[1] / known[1]) : 1;
  const scale = zoom === "fit" ? fill : zoom === "shrink" ? Math.min(fill, 1) : zoom;

  const set = (z: number) => setZoom(fitted(z));

  // A pinch zooms around the point under the fingers: where that point sits in the image, as
  // a fraction of its size, and where it was in the view.
  const anchor = useRef<{ fx: number; fy: number; x: number; y: number }>(undefined);
  const live = useRef({ scale, set });
  live.current = { scale, set };
  /** The zoom a pinch asked for that hasn't shown yet, so quick steps add up. */
  const asked = useRef<number>(undefined);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `src` mounts the view
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    return onPinch(el, (factor, x, y) => {
      const img = el.querySelector("img")?.getBoundingClientRect();
      const view = el.getBoundingClientRect();
      if (img && img.width > 0 && img.height > 0) {
        anchor.current = {
          fx: (x - img.left) / img.width,
          fy: (y - img.top) / img.height,
          x: x - view.left,
          y: y - view.top,
        };
      }
      const next = fitted((asked.current ?? live.current.scale) * factor);
      asked.current = next;
      live.current.set(next);
    });
  }, [src]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once each new zoom shows
  useLayoutEffect(() => {
    const el = box.current;
    const at = anchor.current;
    anchor.current = undefined;
    asked.current = undefined;
    const img = el?.querySelector("img")?.getBoundingClientRect();
    if (!el || !at || !img) return;
    const view = el.getBoundingClientRect();
    el.scrollLeft += img.left - view.left + at.fx * img.width - at.x;
    el.scrollTop += img.top - view.top + at.fy * img.height - at.y;
  }, [scale]);
  const controls = (
    <ZoomControls zoom={scale} setZoom={set} fit={() => setZoom("fit")} fitLabel="Fit" />
  );

  function onKeyDown(e: React.KeyboardEvent) {
    const key = zoomKey(e);
    if (!key) return;
    e.preventDefault();
    e.stopPropagation();
    if (key === "in") set(stepZoom(scale, 1));
    else if (key === "out") set(stepZoom(scale, -1));
    else if (key === "actual") set(1);
    else setZoom("fit");
  }

  const view = src && (
    <div
      ref={box}
      className="image-view"
      // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so the zoom keys and scrolling work
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      <img
        src={src}
        alt={alt}
        draggable={false}
        style={
          known
            ? { width: Math.floor(known[0] * scale), height: Math.floor(known[1] * scale) }
            : { maxWidth: "100%", maxHeight: "100%", visibility: size ? undefined : "hidden" }
        }
        onLoad={(e) => setSize([e.currentTarget.naturalWidth, e.currentTarget.naturalHeight])}
      />
    </div>
  );
  return { controls, view, size, onKeyDown };
}
