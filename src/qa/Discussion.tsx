import { useEffect, useRef, useState } from "react";
import { MarkdownView } from "../editor/MarkdownView";
import { isMac } from "../workbench/keys";
import type { Question } from "./model";

interface Props {
  q: Question;
  /** The agent's reply as it streams in. */
  streaming?: string;
  busy: boolean;
  onSend(message: string): void;
  onReask(): void;
  onClose(): void;
}

/** Talking a question over with the agent. Nothing is decided; ⌘↵ has the agent ask the
 * question again in light of the talk. */
export function Discussion({ q, streaming, busy, onSend, onReask, onClose }: Props) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => ref.current?.focus(), []);

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape") onClose();
    else if (mod && e.key === "Enter") {
      if (!busy && q.discussion.length) onReask();
    } else if (e.key === "Enter" && !e.shiftKey) {
      if (busy || !text.trim()) return e.preventDefault();
      onSend(text.trim());
      setText("");
    } else return;
    e.preventDefault();
    e.stopPropagation();
  }

  return (
    <section className="qq-discussion">
      {q.discussion.map((t, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: turns only append
        <div key={i} className={`qq-turn${t.you ? " you" : ""}`}>
          {t.you ? t.text : <MarkdownView text={t.text} />}
        </div>
      ))}
      {streaming !== undefined && (
        <div className="qq-turn">
          {streaming ? <MarkdownView text={streaming} /> : <span className="faint">Thinking…</span>}
        </div>
      )}
      <textarea
        ref={ref}
        className="qq-input"
        rows={2}
        value={text}
        placeholder={`Ask about ${q.id}, or push back on its options`}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKey}
      />
      <div className="qq-hints">
        <span>
          <kbd>↵</kbd> send
        </span>
        {q.discussion.length > 0 && (
          <span>
            <kbd>⌘↵</kbd> ask {q.id} again
          </span>
        )}
        <span>
          <kbd>esc</kbd> close
        </span>
      </div>
    </section>
  );
}
