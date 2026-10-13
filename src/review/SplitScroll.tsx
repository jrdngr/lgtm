import { type RefObject, useEffect, useMemo, useRef } from "react";
import type { FlatFile } from "./model";
import { splitWidths } from "./width";

const SIDES = ["left", "right"] as const;
/** Pixels per wheel line, for mice that scroll by lines. */
const LINE = 20;

interface Props {
  files: FlatFile[];
  scrollRef: RefObject<HTMLDivElement | null>;
}

/** A scroll bar under each side of a split diff, so the sides scroll sideways on their own.
 * Each bar's offset shifts its side's code. Sideways wheel and trackpad scrolling over the
 * diff moves both bars, and a bar at its end stays put while the other keeps going. */
export function SplitScroll({ files, scrollRef }: Props) {
  const widths = useMemo(() => splitWidths(files), [files]);
  const bars = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      // A mouse wheel with Shift held scrolls sideways.
      const sideways = e.deltaX === 0 && e.shiftKey;
      const dx = (sideways ? e.deltaY : e.deltaX) * (e.deltaMode === 1 ? LINE : 1);
      if (dx === 0) return;
      if (sideways) e.preventDefault();
      for (const bar of bars.current) if (bar) bar.scrollLeft += dx;
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      for (const side of SIDES) el.style.removeProperty(`--${side}-x`);
    };
  }, [scrollRef]);

  return (
    <div className="split-scroll">
      {SIDES.map((side, i) => (
        <div
          key={side}
          ref={(bar) => {
            bars.current[i] = bar;
          }}
          className="split-bar"
          onScroll={(e) =>
            scrollRef.current?.style.setProperty(`--${side}-x`, `${e.currentTarget.scrollLeft}px`)
          }
        >
          <div style={{ width: widths[side], height: 1 }} />
        </div>
      ))}
    </div>
  );
}
