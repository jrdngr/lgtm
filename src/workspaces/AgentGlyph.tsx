import type { Activity } from "./activity";

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

/** A workspace's most urgent agent state as a 12px mark, with a count when it has several
 * agents. */
export function AgentGlyph({ a }: { a: Activity }) {
  const title = a.labels.length ? `${NAMES[a.state]}: ${a.labels.join(", ")}` : NAMES[a.state];
  return (
    <span className="ws-agents" title={title}>
      {a.count > 1 && <span className="ws-count">{a.count}</span>}
      <svg className={`ws-glyph ${a.state}`} viewBox="0 0 12 12" aria-label={NAMES[a.state]}>
        <Shape state={a.state} />
      </svg>
    </span>
  );
}
