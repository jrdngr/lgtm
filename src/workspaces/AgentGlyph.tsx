import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Role } from "../terminal/registry";
import { ago } from "../ui/time";
import type { Activity, AgentInfo } from "./activity";
import "./workspaces.css";

const NAMES = {
  input: "Needs input",
  failed: "Failed",
  stalled: "Stalled",
  working: "Working",
  done: "Done",
  idle: "Idle",
  none: "No agents",
} as const;

function Shape({ state }: { state: Activity["state"] }) {
  switch (state) {
    case "input":
      return (
        <>
          <circle className="ws-halo" cx="6" cy="6" r="5" fill="none" strokeWidth="1.2" />
          <circle cx="6" cy="6" r="3.5" />
        </>
      );
    case "failed":
      return (
        <>
          <circle cx="6" cy="6" r="5.5" />
          <path className="ws-cut" d="M4 4l4 4M8 4l-4 4" strokeWidth="1.6" strokeLinecap="round" />
        </>
      );
    case "stalled":
      return <circle cx="6" cy="6" r="4.5" fill="none" strokeWidth="2" strokeDasharray="2.4 2.3" />;
    case "working":
      return (
        <>
          <circle className="ws-track" cx="6" cy="6" r="4.5" fill="none" strokeWidth="2" />
          <path
            className="ws-spin"
            d="M6 1.5a4.5 4.5 0 0 1 4.5 4.5"
            fill="none"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </>
      );
    case "done":
      return (
        <>
          <circle cx="6" cy="6" r="5.5" />
          <path
            className="ws-cut"
            d="M3.6 6.2l1.7 1.7 3.2-3.5"
            fill="none"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      );
    case "idle":
      return <circle cx="6" cy="6" r="4.2" fill="none" strokeWidth="1.6" />;
    case "none":
      return <circle cx="6" cy="6" r="1.6" />;
  }
}

/** An agent state as a 12px mark. */
export function Glyph({ state, className = "" }: { state: Activity["state"]; className?: string }) {
  return (
    <svg className={`ws-glyph ${state} ${className}`} viewBox="0 0 12 12" aria-label={NAMES[state]}>
      <Shape state={state} />
    </svg>
  );
}

/** How long ago `t` was, in seconds under a minute. */
export function since(t: number) {
  const s = Math.floor((Date.now() - t) / 1000);
  return s < 60 ? `${Math.max(s, 0)}s` : ago(t);
}

/** How long the list stays up after the pointer leaves, to let it reach the list. */
const LEAVE_MS = 150;

interface Props {
  a: Activity;
  /** The agent's name, like its tab's title. */
  name(agent: AgentInfo): string;
  /** The pin of the terminal it runs in, which colors its name. */
  role(agent: AgentInfo): Role | undefined;
  onOpen(agent: AgentInfo): void;
}

/** A workspace's most urgent agent state as a 12px mark. Hovering it lists its agents. */
export function AgentGlyph({ a, name, role, onOpen }: Props) {
  const mark = useRef<HTMLSpanElement>(null);
  const [place, setPlace] = useState<{ top: number; right: number; maxWidth: number }>();
  const leave = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(leave.current), []);
  const shown = place && a.agents.length > 0;
  const enter = () => {
    clearTimeout(leave.current);
    const r = mark.current?.getBoundingClientRect();
    // Grows left from the mark, as wide as its longest name, within the window.
    if (r)
      setPlace({
        top: r.bottom + 4,
        right: window.innerWidth - r.right - 4,
        maxWidth: r.right - 4,
      });
  };
  const out = () => {
    clearTimeout(leave.current);
    leave.current = setTimeout(() => setPlace(undefined), LEAVE_MS);
  };
  return (
    <span className="ws-agents" ref={mark} onPointerEnter={enter} onPointerLeave={out}>
      <Glyph state={a.state} />
      {shown &&
        createPortal(
          <div className="ws-agent-list" style={place} onPointerEnter={enter} onPointerLeave={out}>
            {a.agents.map((g) => (
              <button
                type="button"
                key={g.id}
                className="ws-agent"
                onClick={() => {
                  setPlace(undefined);
                  onOpen(g);
                }}
              >
                <Glyph state={g.state} />
                <span className="ws-agent-name in-terminal" data-send={role(g)}>
                  {name(g)}
                </span>
                <span className="ws-agent-ago">{since(g.since)}</span>
              </button>
            ))}
          </div>,
          document.body,
        )}
    </span>
  );
}
