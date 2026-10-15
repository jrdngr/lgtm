import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CommentDto } from "../bindings";
import { Busy } from "../ui/Busy";
import { quotedReply } from "./prComments";
import type { Comments } from "./useComments";
import "./combine.css";

interface Props {
  /** The quotes to combine, in their first order. */
  quotes: CommentDto[];
  /** Each quote's number on its comment. */
  numbers: Map<number, number>;
  c: Comments;
  onClose(): void;
}

const quoteText = (q: CommentDto) => q.thread?.quote?.text ?? "";

/** One comment on the PR made of several quotes' replies, each under its quote. The replies
 * can be reordered, or the whole comment edited as text, before it's sent. */
export function CombineDialog({ quotes, numbers, c, onClose }: Props) {
  const [order, setOrder] = useState(() => quotes.map((q) => q.id));
  const [text, setText] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState<number>();
  const sendRef = useRef<HTMLButtonElement>(null);
  useEffect(() => sendRef.current?.focus(), []);
  const byId = new Map(quotes.map((q) => [q.id, q]));
  const shown = order.flatMap((id) => byId.get(id) ?? []);
  const combined = shown.map((q) => quotedReply(quoteText(q), q.body)).join("\n\n");

  function move(from: number, to: number) {
    if (to < 0 || to >= order.length) return;
    const next = [...order];
    const [id] = next.splice(from, 1);
    next.splice(to, 0, id);
    setOrder(next);
  }

  async function send() {
    setBusy(true);
    const sent = await c.sendCombined(order, text ?? combined);
    setBusy(false);
    if (sent) onClose();
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    e.stopPropagation();
    onClose();
  }

  // Over the window: blocks sit in transformed rows, which would hold a fixed scrim.
  return createPortal(
    <div
      className="combine-scrim"
      onKeyDown={onKey}
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="combine" role="dialog" aria-labelledby="combine-title">
        <h3 id="combine-title">Combine responses</h3>
        <div className="combine-body">
          {text !== undefined ? (
            <textarea
              className="comment-input combine-text"
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          ) : (
            shown.map((q, i) => (
              <div
                key={q.id}
                className={`combine-item${dragging === i ? " dragging" : ""}`}
                draggable
                onDragStart={() => setDragging(i)}
                onDragEnd={() => setDragging(undefined)}
                onDragOver={(e) => {
                  e.preventDefault();
                  if (dragging !== undefined && dragging !== i) {
                    move(dragging, i);
                    setDragging(i);
                  }
                }}
              >
                <span className="combine-grip">⠿</span>
                <div className="combine-pair">
                  <blockquote className="quote-text">
                    <span className="quote-num">{numbers.get(q.id)}</span>
                    {quoteText(q)}
                  </blockquote>
                  <p>{q.body}</p>
                </div>
                <span className="combine-move">
                  <button type="button" disabled={i === 0} onClick={() => move(i, i - 1)}>
                    ▲
                  </button>
                  <button
                    type="button"
                    disabled={i === shown.length - 1}
                    onClick={() => move(i, i + 1)}
                  >
                    ▼
                  </button>
                </span>
              </div>
            ))
          )}
        </div>
        <div className="combine-foot">
          {text === undefined && (
            <button type="button" onClick={() => setText(combined)}>
              Edit as text
            </button>
          )}
          <span className="spacer" />
          <button type="button" className="quiet" onClick={onClose}>
            Cancel <kbd>Esc</kbd>
          </button>
          <button
            ref={sendRef}
            type="button"
            className="combine-send"
            disabled={busy || !(text ?? combined).trim()}
            onClick={send}
          >
            {busy ? <Busy>Sending…</Busy> : "Send to GitHub"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
