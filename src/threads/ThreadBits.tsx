import { useState } from "react";
import type { Take, ThreadState } from "../bindings";
import { Inline } from "../bots/Markdown";
import { stageName, VERDICTS } from "./stages";

/** The agent's verdict on a thread, as a colored pill. */
export function TakePill({ take }: { take: Take | null }) {
  if (!take) return null;
  const [label, tone] = VERDICTS[take.verdict];
  return <span className={`pill ${tone}`}>{label}</span>;
}

const STAGE_TONES: Record<ThreadState["stage"], string> = {
  new: "amber",
  decide: "amber",
  toFix: "blue",
  reply: "purple",
  ready: "green",
  done: "",
};

export function StagePill({ state }: { state: ThreadState }) {
  return <span className={`pill ${STAGE_TONES[state.stage]}`}>{stageName(state.stage)}</span>;
}

/** "Reply draft" or "Final" over a thread's reply, with a toggle that shows the take behind it
 * in place. */
export function ReplyHead({ state }: { state: ThreadState }) {
  const [open, setOpen] = useState(false);
  const take = state.take;
  return (
    <>
      <div className="reply-head">
        <span className={`pill ${state.stage === "ready" ? "green" : "blue"}`}>
          {state.stage === "ready" ? "Final" : "Reply draft"}
        </span>
        {take && (
          <button
            type="button"
            className="quiet take-toggle"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            {open ? "▾" : "▸"} take
          </button>
        )}
      </div>
      {open && take && (
        <div className="take-why">
          <Inline>{take.why}</Inline>
          {take.question && (
            <div className="take-question">
              <Inline>{take.question}</Inline>
            </div>
          )}
        </div>
      )}
    </>
  );
}
