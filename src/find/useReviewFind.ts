import { type RefObject, useEffect, useMemo, useRef, useState } from "react";
import type { DisplayLine, FlatFile, ViewMode } from "../review/model";
import { paint, textRanges, unpaint } from "./domFind";
import type { FindState } from "./FindBar";
import { stepMatch } from "./findText";
import { findInLines } from "./reviewFind";

interface Deps {
  files: FlatFile[];
  lines: DisplayLine[];
  mode: ViewMode;
  /** The scroller the diff's lines render in. */
  scrollRef: RefObject<HTMLElement | null>;
  scrollTo(to: { line: number; center: boolean }): void;
  /** Hands keys back to the review when the bar closes. */
  focus(): void;
}

/** Find over the review's whole diff. Matches come from the review's data, so the count
 * covers lines not rendered yet; the rendered ones are highlighted as they scroll in. */
export function useReviewFind({ files, lines, mode, scrollRef, scrollTo, focus }: Deps) {
  const [open, setOpen] = useState(false);
  const [focusKey, setFocusKey] = useState(0);
  const [state, setState] = useState<FindState>({ query: "", matchCase: false });
  const [at, setAt] = useState(0);
  const matches = useMemo(
    () => (open ? findInLines(files, lines, mode, state.query, state.matchCase) : []),
    [open, files, lines, mode, state],
  );
  const shown = Math.min(at, Math.max(matches.length - 1, 0));
  const current = matches[shown];
  const atRef = useRef(shown);
  atRef.current = shown;

  // biome-ignore lint/correctness/useExhaustiveDependencies: a new query starts at the top
  useEffect(() => setAt(0), [state]);

  // Brings the current match's line to the middle when it's hidden, including under the
  // sticky file header.
  useEffect(() => {
    const root = scrollRef.current;
    if (!open || !current || !root) return;
    const el = root.querySelector(`[data-index="${current.line}"]`);
    const view = root.getBoundingClientRect();
    const top = root.querySelector(".sticky-file")?.getBoundingClientRect().bottom ?? view.top;
    const box = el?.getBoundingClientRect();
    if (!box || box.top < top || box.bottom > view.bottom)
      scrollTo({ line: current.line, center: true });
  }, [open, current, scrollRef, scrollTo]);

  // Highlights the matches in the lines rendered now, and again as others render.
  useEffect(() => {
    const root = scrollRef.current;
    if (!open || !root) return unpaint();
    const lineSet = new Set(matches.map((m) => m.line));
    let frame = 0;
    const draw = () => {
      frame = 0;
      const ranges: Range[] = [];
      let mine: Range | undefined;
      for (const el of root.querySelectorAll<HTMLElement>("[data-index]")) {
        const line = Number(el.dataset.index);
        if (!lineSet.has(line)) continue;
        const inLine = [...el.querySelectorAll(".code")].flatMap((c) =>
          textRanges(c, state.query, state.matchCase),
        );
        if (current?.line === line) mine = inLine[current.nth];
        ranges.push(...inLine);
      }
      paint(ranges, mine);
    };
    draw();
    const changes = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(draw);
    });
    changes.observe(root, { childList: true, subtree: true });
    return () => {
      changes.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [open, matches, current, state, scrollRef]);

  useEffect(() => unpaint, []);

  return {
    open,
    show() {
      const picked = window.getSelection()?.toString() ?? "";
      if (!open && picked && !picked.includes("\n")) setState((s) => ({ ...s, query: picked }));
      setOpen(true);
      setFocusKey((k) => k + 1);
    },
    bar: {
      ...state,
      at: shown,
      total: matches.length,
      focusKey,
      onChange: setState,
      onStep: (back: boolean) => setAt(stepMatch(atRef.current, matches.length, back)),
      onClose: () => {
        setOpen(false);
        focus();
      },
    },
  };
}
