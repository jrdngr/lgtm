import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type AgentHookState, events } from "../bindings";

type Handler = (state: AgentHookState) => void;

/** What Claude Code's hooks report, by terminal panel id. */
const handlers = new Map<string, Handler>();
let listening = false;

/** Calls `handler` with each state Claude Code's hooks report for terminal `id`, until the
 * returned function is called. */
export function onAgentHook(id: string, handler: Handler): () => void {
  if (!listening) {
    listening = true;
    events
      .terminalAgent(getCurrentWebviewWindow())
      .listen(({ payload }) => handlers.get(payload.terminal)?.(payload.state));
  }
  handlers.set(id, handler);
  return () => {
    if (handlers.get(id) === handler) handlers.delete(id);
  };
}
