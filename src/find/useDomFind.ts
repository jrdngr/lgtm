import { type RefObject, useEffect, useRef, useState } from "react";
import { paint, reveal, textRanges, unpaint } from "./domFind";
import type { FindState } from "./FindBar";
import { stepMatch } from "./findText";

/** Find over the text rendered in `rootRef`, kept current as it changes. */
export function useDomFind(rootRef: RefObject<HTMLElement | null>) {
  const [open, setOpen] = useState(false);
  const [focusKey, setFocusKey] = useState(0);
  const [state, setState] = useState<FindState>({ query: "", matchCase: false });
  const [at, setAt] = useState(0);
  const [ranges, setRanges] = useState<Range[]>([]);
  const atRef = useRef(at);
  atRef.current = at;

  useEffect(() => {
    const root = rootRef.current;
    if (!open || !root) return;
    let frame = 0;
    const find = () => {
      frame = 0;
      setRanges(textRanges(root, state.query, state.matchCase));
    };
    find();
    // Text that changes under the bar, like a reload, is searched again.
    const changes = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(find);
    });
    changes.observe(root, { childList: true, subtree: true, characterData: true });
    return () => {
      changes.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [open, state, rootRef]);

  // A new query starts from the first match.
  // biome-ignore lint/correctness/useExhaustiveDependencies: resets on each query
  useEffect(() => setAt(0), [state]);

  useEffect(() => {
    if (!open) return unpaint();
    const i = Math.min(at, ranges.length - 1);
    const current = ranges[i];
    paint(ranges, current);
    if (current) reveal(current);
  }, [open, ranges, at]);

  useEffect(() => unpaint, []);

  return {
    open,
    /** Opens the bar, or takes focus back to it. */
    show() {
      setOpen(true);
      setFocusKey((k) => k + 1);
    },
    bar: {
      ...state,
      at: Math.min(at, Math.max(ranges.length - 1, 0)),
      total: ranges.length,
      focusKey,
      onChange: setState,
      onStep: (back: boolean) => setAt(stepMatch(atRef.current, ranges.length, back)),
      onClose: () => {
        setOpen(false);
        rootRef.current?.focus();
      },
    },
  };
}
