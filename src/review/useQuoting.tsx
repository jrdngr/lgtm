import { type RefObject, useEffect, useLayoutEffect, useState } from "react";
import type { CommentDto } from "../bindings";
import { quotesOf } from "../model/prComments";
import { markQuotes } from "./quoteMarks";
import type { Comments } from "./useComments";

/** The text selected inside `root`, if the whole selection is in it. */
function selectedIn(root: HTMLElement | null): { text: string; rect: DOMRect } | undefined {
  const sel = window.getSelection();
  if (!root || !sel || sel.isCollapsed || sel.rangeCount === 0) return undefined;
  const range = sel.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return undefined;
  const text = sel.toString().trim();
  return text ? { text, rect: range.getBoundingClientRect() } : undefined;
}

/** Quoting parts of a comment on the PR: a Quote button over text selected in its posts, which
 * `q` presses too, and underlines under what's quoted already. */
export function useQuoting(
  comment: CommentDto,
  c: Comments,
  posts: RefObject<HTMLDivElement | null>,
  block: RefObject<HTMLDivElement | null>,
) {
  const [button, setButton] = useState<{ x: number; y: number; text: string }>();
  const quotes = quotesOf(c.onPr, comment.id).map((q) => q.thread?.quote?.text ?? "");
  const key = quotes.join("\0");

  // biome-ignore lint/correctness/useExhaustiveDependencies: redraw when the quotes or posts change
  useLayoutEffect(() => {
    markQuotes(comment.id, posts.current, quotes);
  }, [key, comment.github]);
  useEffect(() => () => markQuotes(comment.id, null, []), [comment.id]);

  // While text is selected, the button goes with the selection and `q` quotes it, wherever
  // focus is.
  // biome-ignore lint/correctness/useExhaustiveDependencies: quote reads the latest props
  useEffect(() => {
    if (!button) return;
    const gone = () => {
      if (!selectedIn(posts.current)) setButton(undefined);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "q" || e.metaKey || e.ctrlKey || e.altKey) return;
      const s = selectedIn(posts.current);
      if (!s) return;
      e.preventDefault();
      e.stopPropagation();
      quote(s.text);
    };
    document.addEventListener("selectionchange", gone);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("selectionchange", gone);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [button, posts]);

  function quote(text: string) {
    setButton(undefined);
    window.getSelection()?.removeAllRanges();
    c.quote(comment.id, text);
  }

  /** Shows the Quote button over a selection made in the posts. */
  function onMouseUp() {
    const s = selectedIn(posts.current);
    const box = block.current?.getBoundingClientRect();
    if (!s || !box) return setButton(undefined);
    setButton({ x: s.rect.left - box.left, y: s.rect.top - box.top, text: s.text });
  }

  const quoteButton = button && (
    <button
      type="button"
      className="quote-btn"
      style={{ left: Math.max(button.x, 8), top: Math.max(button.y - 30, 4) }}
      // Keeps the selection while clicking.
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => quote(button.text)}
    >
      ❝ Quote <kbd>q</kbd>
    </button>
  );

  return { onMouseUp, quoteButton };
}
