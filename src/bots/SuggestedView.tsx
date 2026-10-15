import { useState } from "react";
import type { BotRun, Finding } from "../bindings";
import { Busy } from "../ui/Busy";
import {
  finalFindings,
  findingsById,
  locationText,
  type Report,
  raisers,
  refLabel,
  verdictOf,
} from "./findings";
import { Inline, Markdown } from "./Markdown";
import { Refs } from "./Refs";
import { SevTag, Tally } from "./Severity";

interface Props {
  run: BotRun;
  r: Report;
  /** The filter is running again for a new task. */
  filtering: boolean;
  /** Indexes of the fixes left out of Send and Fix. */
  skipped: Set<number>;
  onSkip(i: number): void;
  onRetask(task: string): void;
  /** Shows a finding in Findings. */
  onGo(id: string): void;
}

/** The task, then the fixes it needs, the off-task findings that matter, and questions. */
export function SuggestedView({ run, r, filtering, skipped, onSkip, onRetask, onGo }: Props) {
  const m = r.minimal;
  const [editing, setEditing] = useState<string>();
  const [open, setOpen] = useState<Set<number>>(new Set());
  if (!m) return null;
  const byId = findingsById(r);
  const label = (id: string) => refLabel(run, id);
  const verdict = verdictOf(r);
  const task = run.task ?? m.task;
  const fixes = m.fixes ?? [];
  const questions = m.questions ?? [];
  const offTask = m.offTask.flatMap((id) => {
    const f = byId.get(id);
    return f ? [{ id, f }] : [];
  });
  const toggle = (i: number) => {
    const next = new Set(open);
    if (!next.delete(i)) next.add(i);
    setOpen(next);
  };

  return (
    <div className="suggested">
      <div className="suggested-task">
        <span className="suggested-key">Task</span>
        {editing === undefined ? (
          <button
            type="button"
            className="suggested-task-text"
            disabled={filtering || !!run.active}
            onClick={() => setEditing(task)}
          >
            {task}
          </button>
        ) : (
          <input
            // biome-ignore lint/a11y/noAutofocus: it opens on a click to edit
            autoFocus
            className="suggested-task-input"
            value={editing}
            onChange={(e) => setEditing(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && editing.trim()) {
                setEditing(undefined);
                if (editing.trim() !== task) onRetask(editing);
              } else if (e.key === "Escape") {
                e.stopPropagation();
                setEditing(undefined);
              }
            }}
            onBlur={() => setEditing(undefined)}
          />
        )}
        {filtering && <Busy>Filtering…</Busy>}
      </div>
      <div className={filtering ? "suggested-body stale" : "suggested-body"}>
        {verdict && (
          <div className={`suggested-verdict worst-${finalFindings(r)[0]?.severity ?? "none"}`}>
            <div className="suggested-verdict-text">
              <Markdown>{verdict}</Markdown>
            </div>
            <Tally findings={finalFindings(r)} />
          </div>
        )}
        <section>
          <h3>Suggested fixes</h3>
          <ol className="suggested-fixes">
            {fixes.map((a, i) => (
              <li key={a.action} className={skipped.has(i) ? "off" : undefined}>
                <div className="suggested-fix">
                  <button
                    type="button"
                    className="suggested-chevron"
                    aria-expanded={open.has(i)}
                    onClick={() => toggle(i)}
                  >
                    {open.has(i) ? "▾" : "▸"}
                  </button>
                  <span className="suggested-num">{i + 1}.</span>
                  <span className="suggested-action">
                    <Inline>{a.action}</Inline>
                  </span>
                  <input type="checkbox" checked={!skipped.has(i)} onChange={() => onSkip(i)} />
                  <span className="suggested-refs">
                    <Refs ids={a.closes} onGo={onGo} label={label} />
                  </span>
                </div>
                {open.has(i) && (
                  <div className="suggested-closed">
                    {a.closes
                      .flatMap((id) => byId.get(id) ?? [])
                      .map((f) => (
                        <Closed key={f.id} f={f} />
                      ))}
                  </div>
                )}
              </li>
            ))}
          </ol>
        </section>
        {offTask.length > 0 && (
          <section>
            <h3>Off-task, critical or high</h3>
            <div className="suggested-off">
              {offTask.map(({ id, f }) => (
                <div key={id}>
                  <Refs ids={[id]} onGo={onGo} label={label} />
                  <SevTag severity={f.severity} />
                  <Inline>{f.title}</Inline>
                  <span className="suggested-loc">{f.locations.map(locationText).join(", ")}</span>
                </div>
              ))}
            </div>
          </section>
        )}
        {questions.length > 0 && (
          <section>
            <h3>Questions</h3>
            <ol>
              {questions.map((q) => (
                <li key={q.question}>
                  <Inline>{q.question}</Inline> <Refs ids={q.findings} onGo={onGo} label={label} />
                </li>
              ))}
            </ol>
          </section>
        )}
      </div>
    </div>
  );
}

/** A finding a suggested fix closes, in brief. */
function Closed({ f }: { f: Finding }) {
  return (
    <div className="suggested-mini">
      <div>
        <SevTag severity={f.severity} /> <Inline>{f.title}</Inline>
      </div>
      <div className="suggested-loc">
        {f.locations.map(locationText).join(", ")}
        <span className="finding-who"> · {raisers(f).join(", ")}</span>
      </div>
    </div>
  );
}
