import { useEffect, useRef, useState } from "react";
import { fitted, stepZoom } from "./htmlNav";
import { ZoomControls, zoomKey } from "./ZoomControls";

/** An image that fits its tab until zoomed. `controls` go in the tab's bar; `view` shows the
 * image and takes the zoom keys while focused. */
export function useImageZoom(src: string | undefined, alt: string) {
  /** Undefined while fitting the tab. */
  const [zoom, setZoom] = useState<number>();
  /** The scale the fitted image shows at. */
  const [fitScale, setFitScale] = useState(1);
  const [size, setSize] = useState<[number, number]>();
  const img = useRef<HTMLImageElement>(null);
  const scale = zoom ?? fitScale;

  // biome-ignore lint/correctness/useExhaustiveDependencies: `src` mounts the image to measure
  useEffect(() => {
    const el = img.current;
    if (zoom !== undefined || !el) return;
    const measure = () => el.naturalWidth && setFitScale(el.clientWidth / el.naturalWidth);
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [zoom, src]);

  const set = (z: number) => setZoom(fitted(z));
  const controls = (
    <ZoomControls zoom={scale} setZoom={set} fit={() => setZoom(undefined)} fitLabel="Fit" />
  );

  function onKeyDown(e: React.KeyboardEvent) {
    const key = zoomKey(e);
    if (!key) return;
    e.preventDefault();
    e.stopPropagation();
    if (key === "in") set(stepZoom(scale, 1));
    else if (key === "out") set(stepZoom(scale, -1));
    else if (key === "actual") set(1);
    else setZoom(undefined);
  }

  const width = size && zoom !== undefined ? size[0] * zoom : undefined;
  const view = src && (
    <div
      className={`image-view${zoom === undefined ? "" : " zoomed"}`}
      // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so the zoom keys and scrolling work
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      <img
        ref={img}
        src={src}
        alt={alt}
        draggable={false}
        style={width === undefined ? undefined : { width, maxWidth: "none", maxHeight: "none" }}
        onLoad={(e) => setSize([e.currentTarget.naturalWidth, e.currentTarget.naturalHeight])}
      />
    </div>
  );
  return { controls, view, size, onKeyDown };
}
