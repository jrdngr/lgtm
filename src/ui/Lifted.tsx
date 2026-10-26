import { type ComponentProps, useImperativeHandle, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "./lifted.css";

/** Space kept between a lifted popup and the window's edge. */
const EDGE = 6;

/** Over everything, where `At` puts it. */
const LIFTED = {
  position: "fixed",
  right: "auto",
  bottom: "auto",
  margin: 0,
  zIndex: 1000,
} as const;

interface At {
  left: number;
  top: number;
}

/** A popup laid out by its class where it's written, then shown over every pane and panel:
 * portaled to the body, fixed where its class put it, and kept inside the window. */
export function Lifted({ className, style, ref, ...rest }: ComponentProps<"div">) {
  /** An empty, hidden copy of the popup's box in place, which says where the popup goes. */
  const mark = useRef<HTMLSpanElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  useImperativeHandle(ref, () => pop.current as HTMLDivElement, []);
  const [at, setAt] = useState<At>();

  useLayoutEffect(() => {
    const place = () => {
      const m = mark.current;
      const p = pop.current;
      if (!m || !p) return;
      const box = m.getBoundingClientRect();
      const css = getComputedStyle(m);
      const w = p.offsetWidth;
      const h = p.offsetHeight;
      // Anchored on the right or bottom, the popup grows away from that edge.
      const fromRight = css.left === "auto" && css.right !== "auto";
      const fromBottom = css.top === "auto" && css.bottom !== "auto";
      let left = fromRight ? box.right - w : box.left;
      let top = fromBottom ? box.bottom - h : box.top;
      left = Math.max(EDGE, Math.min(left, window.innerWidth - w - EDGE));
      top = Math.max(EDGE, Math.min(top, window.innerHeight - h - EDGE));
      setAt((a) => (a && a.left === left && a.top === top ? a : { left, top }));
    };
    place();
    const size = new ResizeObserver(place);
    if (pop.current) size.observe(pop.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      size.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, []);

  return (
    <>
      <span ref={mark} className={`${className ?? ""} lifted-mark`} style={style} aria-hidden />
      {createPortal(
        <div
          ref={pop}
          className={className}
          style={{ ...style, ...LIFTED, ...(at ?? { visibility: "hidden" }) }}
          {...rest}
        />,
        document.body,
      )}
    </>
  );
}
