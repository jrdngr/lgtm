import type { DockviewPanelApi } from "dockview-react";
import { type RefObject, useEffect, useRef, useState } from "react";
import type { DisplayLine, FlatFile } from "./model";
import { findLine, lineRef, parseView, type SavedView } from "./savedView";
import type { useCollapse } from "./useCollapse";

/** Comment blocks folded or unfolded by hand, kept for the state they were in. */
export interface CommentFolds {
  get(id: number, state: string): boolean | undefined;
  set(id: number, folded: boolean, state: string): void;
}

/** How many comment folds are kept, newest first. */
const MAX_COMMENT_FOLDS = 200;
const WRITE_DELAY = 400;

interface View {
  api: DockviewPanelApi;
  params: { view?: unknown } | undefined;
  review: number | undefined;
  revset: string;
  files: FlatFile[];
  lines: DisplayLine[];
  cursor: number;
  fold: ReturnType<typeof useCollapse>;
  scrollRef: RefObject<HTMLDivElement | null>;
  place(line: number): void;
  setScrollTo(to: { line: number }): void;
}

const firstRow = (lines: DisplayLine[]) =>
  Math.max(
    lines.findIndex((l) => l.kind === "rows"),
    0,
  );

/** The index of the first line showing at the top of the diff. */
function topIndex(el: HTMLElement): number | undefined {
  const top = el.getBoundingClientRect().top;
  let best: number | undefined;
  for (const line of el.querySelectorAll<HTMLElement>(".line[data-index]")) {
    if (line.getBoundingClientRect().bottom <= top) continue;
    const i = Number(line.dataset.index);
    if (best === undefined || i < best) best = i;
  }
  return best;
}

/**
 * Keeps the review's folds, cursor and scroll in its panel's params, which the workspace's
 * layout saves, and brings them back when the first review after a restart is on the same
 * revset. A newly opened review otherwise starts on its first row. Writes wait for scrolling
 * and toggling to settle.
 */
export function useSavedView(v: View): CommentFolds {
  const [initial] = useState(() => parseView(v.params?.view));
  const pending = useRef(initial);
  /** Set once the first review is in, so the empty state before it isn't saved. */
  const ready = useRef(false);
  const [restoring, setRestoring] = useState<SavedView>();
  const [folds, setFolds] = useState<ReadonlyMap<number, [boolean, string]>>(
    () => new Map(initial?.comments.map(([id, folded, state]) => [id, [folded, state]])),
  );
  const top = useRef(initial?.top);
  const written = useRef<string>(undefined);
  const timer = useRef<number>(undefined);
  const latest = useRef(v);
  latest.current = v;
  const latestFolds = useRef(folds);
  latestFolds.current = folds;

  function write() {
    const { api, revset, files, lines, cursor, fold, scrollRef } = latest.current;
    const el = scrollRef.current;
    // A hidden tab's content is out of the page and keeps the last top.
    if (el?.isConnected && el.clientHeight > 0)
      top.current = el.scrollTop > 0 ? lineRef(files, lines[topIndex(el) ?? -1]) : undefined;
    const view: SavedView = {
      revset,
      ...fold.saved(),
      comments: [...latestFolds.current].map(([id, [folded, state]]) => [id, folded, state]),
      cursor: lineRef(files, lines[cursor]),
      top: top.current,
    };
    const json = JSON.stringify(view);
    if (json === written.current) return;
    written.current = json;
    api.updateParameters({ view });
  }

  function schedule() {
    if (!ready.current) return;
    clearTimeout(timer.current);
    timer.current = window.setTimeout(write, WRITE_DELAY);
  }

  useEffect(() => () => clearTimeout(timer.current), []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: save whenever these change
  useEffect(schedule, [v.revset, v.lines, v.cursor, folds]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the diff's element comes with a review
  useEffect(() => {
    const el = v.scrollRef.current;
    if (!el) return;
    el.addEventListener("scroll", schedule, { passive: true });
    return () => el.removeEventListener("scroll", schedule);
  }, [v.review, v.scrollRef]);

  // A newly opened review starts on its first row, or with the folds it had before a restart.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the review is replaced
  useEffect(() => {
    if (v.review === undefined) return;
    const view = pending.current;
    pending.current = undefined;
    ready.current = true;
    if (view?.revset === v.revset) {
      v.fold.restore(view);
      setRestoring(view);
    } else {
      top.current = undefined;
      v.place(firstRow(v.lines));
    }
  }, [v.review]);

  // Then the cursor and scroll go back, once the restored folds are drawn.
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per restore
  useEffect(() => {
    if (!restoring) return;
    setRestoring(undefined);
    const cursor = findLine(v.files, v.lines, restoring.cursor);
    v.place(cursor >= 0 ? cursor : firstRow(v.lines));
    const line = findLine(v.files, v.lines, restoring.top);
    if (line >= 0) v.setScrollTo({ line });
  }, [restoring]);

  return {
    get(id, state) {
      const fold = folds.get(id);
      return fold?.[1] === state ? fold[0] : undefined;
    },
    set(id, folded, state) {
      const next = new Map(folds);
      next.delete(id);
      next.set(id, [folded, state]);
      for (const old of next.keys()) {
        if (next.size <= MAX_COMMENT_FOLDS) break;
        next.delete(old);
      }
      setFolds(next);
    },
  };
}
