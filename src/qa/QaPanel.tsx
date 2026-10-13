import { useState } from "react";
import { IconButton } from "../ui/icons";
import { useWorkspace } from "../workspaces/context";
import { countable, openQuestions, type Session } from "./model";
import { busyName, showQa, startFresh, useQa } from "./store";
import "./qa.css";

/** Where an active session is, in a few words. */
function status(d: Session) {
  const open = openQuestions(d).length;
  const all = countable(d).length;
  if (!all) return "Not asked yet";
  if (open) return `${all - open} of ${all} decided`;
  if (d.sent != null) return "Sent to a terminal";
  if (d.plan) return "Planned";
  return "Ready to apply";
}

function Row({ d, current, busyLabel }: { d: Session; current: boolean; busyLabel?: string }) {
  const ws = useWorkspace();
  const open = openQuestions(d).length;
  return (
    <div className={`qa-row${current ? " current" : ""}`}>
      <button type="button" className="qa-row-name" onClick={() => showQa(ws, d.id)}>
        <span>{d.title}</span>
        {open > 0 && <span className="pill amber">{open} open</span>}
        <span className="spacer" />
        <span className="faint">
          {busyLabel ??
            (d.archived
              ? new Date(d.applied?.at ?? d.created ?? 0).toLocaleDateString()
              : status(d))}
        </span>
      </button>
    </div>
  );
}

/** The workspace's Q&A sessions: active ones with where they are, archived ones folded. */
export function QaPanel() {
  const ws = useWorkspace();
  const state = useQa(ws);
  const [showArchived, setShowArchived] = useState(false);
  const active = state.sessions.filter((d) => !d.archived);
  const archived = state.sessions.filter((d) => d.archived);
  const row = (d: Session) => {
    const busy = state.busy[d.id];
    return (
      <Row
        key={d.id}
        d={d}
        current={!state.fresh && d.id === state.current}
        busyLabel={busy && busyName(busy)}
      />
    );
  };
  return (
    <div className="qa-panel">
      <div className="qa-panel-head">
        <b>Q&amp;A</b>
        <span className="spacer" />
        <IconButton
          icon="plus"
          label="New Q&A"
          onClick={() => {
            startFresh(ws);
            showQa(ws);
          }}
        />
      </div>
      {state.loaded && !active.length && (
        <div className="faint qa-panel-empty">
          No active sessions. + starts one from a prompt; the agent asks what it needs before it
          writes.
        </div>
      )}
      {active.map(row)}
      {archived.length > 0 && (
        <>
          <button
            type="button"
            className="qa-row-name qa-folded"
            onClick={() => setShowArchived(!showArchived)}
          >
            {showArchived ? "▾" : "▸"} Archived ({archived.length})
          </button>
          {showArchived && archived.map(row)}
        </>
      )}
      {state.error && <div className="review-error">{state.error}</div>}
    </div>
  );
}
