import { useEffect, useRef, useState } from "react";
import type { PlanComment, PlanFile } from "../bindings";
import { Inline } from "../bots/Markdown";
import { openFile } from "../editor/open";
import type { WorkspaceName } from "../workspaces/context";
import { edit, writePlan } from "./actions";
import type { Session } from "./model";
import type { Busy } from "./store";

const PILLS: Record<PlanFile["change"], string> = { new: "green", edit: "blue", delete: "red" };

/** The text a comment's quote is matched against. */
const cardText = (f: PlanFile) => [f.path, ...f.edits].join("\n");

interface Props {
  ws: WorkspaceName;
  d: Session;
  busy?: Busy;
  onApply(): void;
}

/** Matches `.qa-pop` in qa.css. */
const POP_WIDTH = 320;

/** Where the write would go, file by file. Selecting text comments on it; Revise sends the
 * comments back to the agent. */
export function PlanView({ ws, d, busy, onApply }: Props) {
  const [pop, setPop] = useState<{ quote: string; top: number; left: number }>();
  const [body, setBody] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const open = pop !== undefined;

  // The popover stays where it opened, so it closes rather than drift from its text.
  useEffect(() => {
    if (!open) return;
    const shut = () => setPop(undefined);
    // The comment input scrolls sideways as it fills, which isn't the view moving.
    const onScroll = (e: Event) => {
      if (!popRef.current?.contains(e.target as Node)) shut();
    };
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", shut);
    return () => {
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", shut);
    };
  }, [open]);

  const plan = d.plan;
  if (!plan) return null;
  const comments = plan.comments ?? [];
  const locked = !!busy || d.archived;
  const placed = new Set<number>();
  const commentsOn = (f: PlanFile) =>
    comments.filter((c) => {
      if (placed.has(c.id) || !cardText(f).includes(c.quote)) return false;
      placed.add(c.id);
      return true;
    });
  const cards = plan.files.map((f) => [f, commentsOn(f)] as const);
  const loose = comments.filter((c) => !placed.has(c.id));
  const from = [...new Set(plan.files.flatMap((f) => f.decisions))];

  function onMouseUp() {
    if (locked) return;
    const sel = window.getSelection();
    const quote = sel?.toString().trim();
    if (!sel || !quote || !ref.current?.contains(sel.anchorNode)) return;
    const r = sel.getRangeAt(0).getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left, window.innerWidth - POP_WIDTH - 8));
    setPop({ quote, top: r.bottom + 4, left });
    setBody("");
  }

  async function save() {
    if (!pop || !body.trim()) return;
    const r = await edit(ws, d, { kind: "planComment", quote: pop.quote, body: body.trim() });
    if (r.status === "ok") setPop(undefined);
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "r" && !locked && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      writePlan(ws, d);
    }
  }

  const comment = (c: PlanComment) => (
    <div key={c.id} className="qa-comment">
      <span className="qa-quote">{c.quote}</span>
      <span>
        <b>You:</b> {c.body}
      </span>
      {!locked && (
        <button
          type="button"
          className="quiet"
          aria-label="Delete comment"
          onClick={() => edit(ws, d, { kind: "deletePlanComment", id: c.id })}
        >
          ×
        </button>
      )}
    </div>
  );

  return (
    <section className="qa-card qa-plan" ref={ref} onMouseUp={onMouseUp} onKeyDown={onKey}>
      <h2>
        Plan
        <span className="faint">
          {plan.files.length} {plan.files.length === 1 ? "file" : "files"}
          {from.length > 0 && ` · from ${from.join(", ")}`} · select text to comment
        </span>
      </h2>
      {cards.map(([f, comments]) => (
        <div key={f.path} className="qa-plan-file">
          <div className="qa-plan-path">
            <span className={`pill ${PILLS[f.change]}`}>{f.change}</span>
            {f.change === "new" ? (
              <code>{f.path}</code>
            ) : (
              <button type="button" className="quiet" onClick={() => openFile(ws, f.path)}>
                <code>{f.path}</code>
              </button>
            )}
          </div>
          <ul>
            {f.edits.map((e) => (
              <li key={e}>
                <Inline>{e}</Inline>
              </li>
            ))}
          </ul>
          {f.decisions.length > 0 && <div className="faint">From {f.decisions.join(", ")}</div>}
          {comments.map(comment)}
        </div>
      ))}
      {loose.map(comment)}
      {pop && (
        <div ref={popRef} className="qa-pop" style={{ top: pop.top, left: pop.left }}>
          <input
            // biome-ignore lint/a11y/noAutofocus: the comment is what the selection asked for
            autoFocus
            className="qq-input"
            value={body}
            placeholder="Comment, then ↵"
            onChange={(e) => setBody(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") save();
              else if (e.key === "Escape") setPop(undefined);
            }}
            onBlur={() => !body.trim() && setPop(undefined)}
          />
        </div>
      )}
      {!d.archived && (
        <div className="qq-actions">
          <button
            type="button"
            disabled={locked}
            onClick={() => edit(ws, d, { kind: "discardPlan" })}
          >
            Discard plan
          </button>
          <span className="spacer" />
          <button type="button" disabled={locked} onClick={() => writePlan(ws, d)}>
            Revise plan <kbd>r</kbd>
          </button>
          <button type="button" className="primary" disabled={locked} onClick={onApply}>
            Apply plan <kbd>⌘↵</kbd>
          </button>
        </div>
      )}
    </section>
  );
}
