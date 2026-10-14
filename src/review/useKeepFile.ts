import type { Virtualizer } from "@tanstack/react-virtual";
import { type RefObject, useLayoutEffect, useRef } from "react";
import type { DisplayLine } from "./model";

const headerOf = (lines: DisplayLine[], file: number) =>
  lines.findIndex((l) => l.kind === "file" && l.file === file);

/**
 * Keeps a file's header where it is on screen while its file collapses or expands, so the
 * lines below it move instead. `keep(file)` notes where the header is now, its sticky spot at
 * the top when it has scrolled away; the next change of `lines` scrolls it back there.
 * `holding` is set until the next frame, so the cursor's own scroll leaves it alone.
 */
export function useKeepFile(
  lines: DisplayLine[],
  virtualizer: Virtualizer<HTMLDivElement, Element>,
  scrollRef: RefObject<HTMLDivElement | null>,
) {
  const pending = useRef<{ file: number; top: number }>(undefined);
  const holding = useRef(false);

  useLayoutEffect(() => {
    const p = pending.current;
    const el = scrollRef.current;
    if (!p || !el) return;
    pending.current = undefined;
    const start = virtualizer.measurementsCache[headerOf(lines, p.file)]?.start;
    if (start === undefined) return;
    el.scrollTop = start - p.top;
    holding.current = true;
    requestAnimationFrame(() => {
      holding.current = false;
    });
  }, [lines, virtualizer, scrollRef]);

  function keep(file: number) {
    const el = scrollRef.current;
    const start = virtualizer.measurementsCache[headerOf(lines, file)]?.start;
    if (!el || start === undefined) return;
    pending.current = { file, top: Math.max(0, start - el.scrollTop) };
  }

  return { keep, holding };
}
