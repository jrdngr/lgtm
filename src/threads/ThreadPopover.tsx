import { useEffect } from "react";
import { createPortal } from "react-dom";
import type { ThreadDto } from "../bindings";
import { threadLabel, threadWhere } from "../model/stages";

interface Props {
  t: ThreadDto;
  /** The row it shows over. */
  rect: DOMRect;
  /** The take's dot class. */
  take: string;
  /** A quote, indented under the comment it's from. */
  sub?: boolean;
  /** Hides it. */
  onLeave(): void;
}

/** A thread's row again over it, wider and two lines tall: its whole text and id, then who
 * wrote it, where, and whether an agent fixed it. */
export function ThreadPopover({ t, rect, take, sub, onLeave }: Props) {
  useEffect(() => {
    window.addEventListener("scroll", onLeave, true);
    return () => window.removeEventListener("scroll", onLeave, true);
  }, [onLeave]);
  const { left, top, width } = rect;
  const author = t.github && !t.state.quote ? t.github.posts[0]?.author : undefined;
  return createPortal(
    <div
      className={`thread-pop${sub ? " sub" : ""}`}
      style={{ left, top, minWidth: width, maxWidth: `calc(100vw - ${left + 16}px)` }}
    >
      <div className="tp-line">
        <span className={`take-dot ${take}`} />
        {!t.github && !t.state.quote && <span className="pill purple">Draft</span>}
        <span className="tp-text">{threadLabel(t)}</span>
        <span className="thread-num">#{t.id}</span>
      </div>
      {(author || t.path || t.fixes.length > 0) && (
        <div className="tp-line tp-meta">
          {author && <b className="thread-who">{author}</b>}
          {t.path && <span className="tp-where">{threadWhere(t)}</span>}
          {t.fixes.length > 0 && (
            <svg className="fix-ic" viewBox="0 0 16 16" aria-label="Fixed">
              <path d="M8 2.5v6M5 5.5h6M5 13h6" />
            </svg>
          )}
        </div>
      )}
    </div>,
    document.body,
  );
}
