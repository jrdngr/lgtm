import { useState } from "react";
import { Inline } from "../bots/Markdown";
import { IconButton } from "../ui/icons";
import { useWorkspace } from "../workspaces/context";
import { decide } from "./actions";
import { type Design, openQuestions, STEP_NAMES, STEP_VIEW, step } from "./model";
import { NewDesign } from "./NewDesign";
import { busyName, showDesign, useDesigns } from "./store";
import "./design.css";

/** The design's next question, small, to settle with its recommendation from here. */
function Next({ d, locked }: { d: Design; locked: boolean }) {
  const ws = useWorkspace();
  const q = openQuestions(d)[0];
  if (!q) return null;
  const rec = q.text.options[0];
  return (
    <div className="design-next">
      <button
        type="button"
        className="design-next-subject"
        onClick={() => showDesign(ws, d.slug, "questions")}
      >
        <b>{q.id}</b> <Inline>{q.text.subject}</Inline>
      </button>
      {rec && (
        <div className="design-next-rec">
          <span className="pill green">1</span> <Inline>{rec.label}</Inline>
        </div>
      )}
      <div className="design-next-actions">
        <button
          type="button"
          className="primary"
          disabled={locked}
          onClick={() => decide(ws, d, q.id, { option: 0, note: null, other: null })}
        >
          Go with 1
        </button>
        <button type="button" onClick={() => showDesign(ws, d.slug, "questions")}>
          Open
        </button>
      </div>
    </div>
  );
}

/** The workspace's designs, where each is in its loop, and the current one's next question. */
export function DesignPanel() {
  const ws = useWorkspace();
  const state = useDesigns(ws);
  const [creating, setCreating] = useState(false);
  return (
    <div className="design-panel">
      <div className="design-panel-head">
        <b>Designs</b>
        <span className="spacer" />
        <IconButton icon="plus" label="New design" onClick={() => setCreating(!creating)} />
      </div>
      {(creating || (state.loaded && !state.designs.length)) && (
        <NewDesign
          ws={ws}
          onDone={() => {
            setCreating(false);
            showDesign(ws);
          }}
        />
      )}
      {state.designs.map((d) => {
        const busy = state.busy[d.slug];
        const open = openQuestions(d).length;
        const s = step(d);
        return (
          <div key={d.slug} className={`design-row${d.slug === state.current ? " current" : ""}`}>
            <button
              type="button"
              className="design-row-name"
              onClick={() => showDesign(ws, d.slug, STEP_VIEW[s])}
            >
              <span>{d.name}</span>
              {open > 0 && <span className="pill amber">{open}</span>}
              <span className="spacer" />
              <span className="faint">{busy ? busyName(busy) : STEP_NAMES[s]}</span>
            </button>
            {d.slug === state.current && <Next d={d} locked={!!busy} />}
          </div>
        );
      })}
      {state.error && <div className="review-error">{state.error}</div>}
    </div>
  );
}
