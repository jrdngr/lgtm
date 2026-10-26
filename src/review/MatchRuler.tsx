import { type RefObject, useEffect, useRef } from "react";
import type { DisplayLine, FlatFile } from "./model";
import { matchOffsets } from "./selectionMatches";

const WIDTH = 10;

/** The lines whose code holds `needle`, by index. */
export function matchedLines(files: FlatFile[], lines: DisplayLine[], needle: string): number[] {
  const out: number[] = [];
  lines.forEach((line, i) => {
    const texts =
      line.kind === "rows"
        ? [...line.rows, line.left, line.right].map((r) =>
            r === undefined ? "" : files[line.file].rows[r].text,
          )
        : line.kind === "context"
          ? [line.line.text]
          : [];
    if (texts.some((t) => matchOffsets(t, needle).length)) out.push(i);
  });
  return out;
}

/** A strip down the review's right edge with a white tick at each line holding the selected
 * text and a band for what's on screen. A click scrolls there. */
export function MatchRuler({
  starts,
  total,
  scrollRef,
}: {
  /** Where each matched line starts, in the scrolled content. */
  starts: number[];
  total: number;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const scroller = scrollRef.current;
    const c = canvas.current;
    if (!scroller || !c) return;
    const draw = () => {
      const height = scroller.clientHeight;
      const dpr = window.devicePixelRatio || 1;
      c.style.height = `${height}px`;
      c.width = WIDTH * dpr;
      c.height = height * dpr;
      const g = c.getContext("2d");
      if (!g) return;
      g.scale(dpr, dpr);
      const y = (top: number) => (top / Math.max(total, 1)) * height;
      g.fillStyle = "rgb(255 255 255 / 9%)";
      g.fillRect(0, y(scroller.scrollTop), WIDTH, Math.max(y(height), 4));
      g.fillStyle = "#fff";
      for (const s of starts) g.fillRect(1, y(s), WIDTH - 2, 2);
    };
    draw();
    scroller.addEventListener("scroll", draw);
    const resize = new ResizeObserver(draw);
    resize.observe(scroller);
    return () => {
      scroller.removeEventListener("scroll", draw);
      resize.disconnect();
    };
  }, [starts, total, scrollRef]);

  return (
    <div className="match-ruler-anchor">
      <canvas
        ref={canvas}
        className="match-ruler"
        onMouseDown={(e) => {
          e.preventDefault();
          const scroller = scrollRef.current;
          if (!scroller) return;
          const r = e.currentTarget.getBoundingClientRect();
          const at = ((e.clientY - r.top) / r.height) * scroller.scrollHeight;
          scroller.scrollTo({ top: at - scroller.clientHeight / 2 });
        }}
      />
    </div>
  );
}
