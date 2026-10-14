import { type RefObject, useEffect } from "react";

/** Puts the diff's scroll back when its tab is shown again. Dockview takes a hidden tab's
 * content out of the page, which resets its scroll, while the virtualizer keeps drawing rows
 * for the old offset, so the pane looked blank. */
export function useKeepScroll(scrollRef: RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    let top = el.scrollTop;
    let left = el.scrollLeft;
    const save = () => {
      if (!el.isConnected) return;
      top = el.scrollTop;
      left = el.scrollLeft;
    };
    // Back in the page, the element gets its size again.
    const restore = () => {
      if (!el.isConnected || el.clientHeight === 0) return;
      if (el.scrollTop !== top) el.scrollTop = top;
      if (el.scrollLeft !== left) el.scrollLeft = left;
    };
    el.addEventListener("scroll", save, { passive: true });
    const observer = new ResizeObserver(restore);
    observer.observe(el);
    return () => {
      el.removeEventListener("scroll", save);
      observer.disconnect();
    };
  }, [scrollRef]);
}
