import { useEffect, useState } from "react";
import { commands, errorText, unwrap } from "../ipc";
import { useAgentsOff } from "./agents";
import { refreshMcpStatus, useMcpStatus } from "./mcp";
import "./tools.css";
import { Busy } from "../ui/Busy";

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
  const status = useMcpStatus();
  const [hidden, setHidden] = useState(askedNever);
  const [state, setState] = useState<"idle" | "working" | "done">("idle");
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (hidden || agentsOff) return;
    refreshMcpStatus().catch(() => {});
  }, [hidden, agentsOff]);

  if (hidden || agentsOff) return null;
  // Registering here keeps the banner up to say so; registering from Settings hides it.
  if (state !== "done" && status?.setup !== "missing") return null;

  async function register() {
    setState("working");
    setError(undefined);
    try {
      await unwrap(commands.registerMcp());
      setState("done");
      refreshMcpStatus().catch(() => {});
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
      <span title={status?.command}>
        Let Claude Code in lgtm's terminals read this review and add comments and findings? This
        registers lgtm as an MCP server for your user.
      </span>
      {error && <span className="error">{error}</span>}
      <span className="spacer" />
      <button type="button" onClick={register} disabled={state === "working"}>
        {state === "working" ? <Busy>Setting up…</Busy> : "Set up"}
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
