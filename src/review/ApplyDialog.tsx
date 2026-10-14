import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CommentDto } from "../bindings";
import { FileDiff } from "./FileDiff";
import type { Comments } from "./useComments";

/** Asks, when a reply with proposed changes goes final, whether to write them. Each changed
 * file opens to its diff. */
export function ApplyDialog({ comment, c }: { comment: CommentDto; c: Comments }) {
  const files = (comment.thread?.proposal ?? []).filter((p) => p.state === "proposed");
  const [open, setOpen] = useState<ReadonlySet<string>>(
    new Set(files.slice(0, 1).map((p) => p.path)),
  );
  const apply = useRef<HTMLButtonElement>(null);
  useEffect(() => apply.current?.focus(), []);
  const id = comment.id;
  const toggle = (path: string) =>
    setOpen((o) => {
      const next = new Set(o);
      if (!next.delete(path)) next.add(path);
      return next;
    });

  function onKey(e: React.KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") c.cancelFinal();
    else if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) c.finish(id, true);
    else return;
    e.preventDefault();
  }

  return createPortal(
    <div
      className="prompt-backdrop"
      onPointerDown={(e) => e.target === e.currentTarget && c.cancelFinal()}
    >
      <div
        className="prompt apply-dialog"
        role="dialog"
        aria-label="Apply changes"
        onKeyDown={onKey}
      >
        <h3>Apply the agent's changes?</h3>
        <p className="faint">Marking the reply final writes these to @.</p>
        <div className="apply-files">
          {files.map((p) => (
            <FileDiff
              key={p.path}
              p={p}
              folded={!open.has(p.path)}
              onToggle={() => toggle(p.path)}
            />
          ))}
        </div>
        <div className="prompt-buttons">
          <button type="button" className="quiet" onClick={c.cancelFinal}>
            Cancel <kbd>Esc</kbd>
          </button>
          <button type="button" onClick={() => c.finish(id, false)}>
            Final without changes
          </button>
          <button ref={apply} type="button" className="apply-go" onClick={() => c.finish(id, true)}>
            Apply and finalize <kbd>↵</kbd>
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
