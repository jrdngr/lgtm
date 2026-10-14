import type { Question, UndoKind } from "./model";

/** What's being written under the question. */
export type Mode = "note" | "other" | "discuss";

const UNDO_LABELS: Record<UndoKind, (id: string) => string> = {
  decision: (id) => `Undo ${id}`,
  defer: () => "Undo defer",
  reopen: (id) => `Undo reopen ${id}`,
};

interface Props {
  q: Question;
  /** The question to answer, when it isn't `q`. */
  next?: Question;
  /** `q` is an open question past `next`, so it can only be looked at. */
  ahead: boolean;
  /** `q` is an auto answer. */
  auto: boolean;
  canAnswer: boolean;
  agentBusy: boolean;
  /** The option picked out. */
  sel: number;
  /** What undo would take back. */
  last?: { id: string; kind: UndoKind };
  /** There's more than one question to browse. */
  browse: boolean;
  onBack(): void;
  onDecide(): void;
  onMode(mode: Mode): void;
  onDefer(): void;
  onUndo(): void;
  onReopen(): void;
}

/** The action bar under the question: primary first, keys inside the buttons. */
export function ActionBar({ q, next, ahead, auto, canAnswer, agentBusy, sel, last, ...p }: Props) {
  if (auto)
    return (
      <div className="qq-actions">
        {next && (
          <button type="button" className="qq-btn primary" onClick={p.onBack}>
            Back to {next.id} <kbd>esc</kbd>
          </button>
        )}
        <button type="button" className="qq-btn cyan" disabled={agentBusy} onClick={p.onReopen}>
          Answer it myself <kbd>r</kbd>
        </button>
        <span className="spacer" />
        <Browse label="browse" />
      </div>
    );
  if (ahead && next)
    return (
      <div className="qq-actions">
        <button type="button" className="qq-btn primary" onClick={p.onBack}>
          Back to {next.id} <kbd>esc</kbd>
        </button>
        <span className="spacer" />
        <Browse label="look around" />
      </div>
    );
  return (
    <div className="qq-actions">
      {q.state === "open" && (
        <>
          <button
            type="button"
            className="qq-btn primary"
            disabled={!canAnswer}
            onClick={p.onDecide}
          >
            {sel === 0 ? "Take recommended" : `Go with ${sel + 1}`} <kbd>↵</kbd>
          </button>
          <span className="qq-group">
            <button
              type="button"
              className="qq-btn"
              disabled={!canAnswer}
              onClick={() => p.onMode("note")}
            >
              Add note <kbd>n</kbd>
            </button>
            <button
              type="button"
              className="qq-btn"
              disabled={!canAnswer}
              onClick={() => p.onMode("other")}
            >
              Other answer <kbd>o</kbd>
            </button>
          </span>
        </>
      )}
      {q.state !== "dropped" && q.state !== "deferred" && (
        <button type="button" className="qq-btn" onClick={() => p.onMode("discuss")}>
          Discuss <kbd>d</kbd>
        </button>
      )}
      {q.state === "open" && (
        <button type="button" className="qq-btn ghost" disabled={!canAnswer} onClick={p.onDefer}>
          Defer <kbd>s</kbd>
        </button>
      )}
      <span className="spacer" />
      {last && (
        <button type="button" className="qq-btn ghost" disabled={agentBusy} onClick={p.onUndo}>
          {UNDO_LABELS[last.kind](last.id)} <kbd>u</kbd>
        </button>
      )}
      {p.browse && <Browse label="browse" />}
    </div>
  );
}

function Browse({ label }: { label: string }) {
  return (
    <span className="qq-browse">
      <kbd>[</kbd>
      <kbd>]</kbd> {label}
    </span>
  );
}
