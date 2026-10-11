import { useEffect, useState } from "react";
import type { McpStatus } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import { useAgentsOff } from "./agents";
import "./tools.css";

const NEVER = "lgtm:mcp-offer:never";

function askedNever() {
  try {
    return localStorage.getItem(NEVER) === "1";
  } catch {
    return false;
  }
}

/** Offers to register lgtm's MCP server with Claude Code, so `claude` in lgtm's terminals
 * can read the review and add to it. Shown until it's set up or turned down. */
export function McpOffer() {
  const agentsOff = useAgentsOff();
  const [status, setStatus] = useState<McpStatus>();
  const [hidden, setHidden] = useState(askedNever);
  const [state, setState] = useState<"idle" | "working" | "done">("idle");
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (hidden || agentsOff) return;
    unwrap(commands.mcpStatus()).then(setStatus, () => {});
  }, [hidden, agentsOff]);

  if (hidden || agentsOff || status?.setup !== "missing") return null;

  async function register() {
    setState("working");
    setError(undefined);
    try {
      await unwrap(commands.registerMcp());
      setState("done");
    } catch (e) {
      setState("idle");
      setError(errorText(e));
    }
  }

  function never() {
    try {
      localStorage.setItem(NEVER, "1");
    } catch {
      // Then it asks again next launch.
    }
    setHidden(true);
  }

  if (state === "done") {
    return (
      <div className="mcp-offer">
        <span>
          Done. New <code>claude</code> sessions in lgtm terminals can use lgtm's tools.
        </span>
        <span className="spacer" />
        <button type="button" onClick={() => setHidden(true)}>
          OK
        </button>
      </div>
    );
  }
  return (
    <div className="mcp-offer">
      <span title={status.command}>
        Let Claude Code in lgtm's terminals read this review and add comments, hotspots and
        findings? This registers lgtm as an MCP server for your user.
      </span>
      {error && <span className="error">{error}</span>}
      <span className="spacer" />
      <button type="button" onClick={register} disabled={state === "working"}>
        {state === "working" ? "Setting up…" : "Set up"}
      </button>
      <button type="button" onClick={() => setHidden(true)}>
        Not now
      </button>
      <button type="button" onClick={never}>
        Don't ask again
      </button>
    </div>
  );
}
