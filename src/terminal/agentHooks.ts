import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type AgentHookState, events, type TerminalAgent } from "../bindings";

type Handler = (state: AgentHookState) => void;

/** What Claude Code's hooks report, by terminal panel id. */
const handlers = new Map<string, Handler>();
/** Handlers that hear every terminal's reports. */
const watchers = new Set<(report: TerminalAgent) => void>();
let listening = false;

function listen() {
  if (listening) return;
  listening = true;
  events.terminalAgent(getCurrentWebviewWindow()).listen(({ payload }) => {
    handlers.get(payload.terminal)?.(payload.state);
    for (const w of watchers) w(payload);
  });
}

/** Calls `handler` with each state Claude Code's hooks report for terminal `id`, until the
 * returned function is called. */
export function onAgentHook(id: string, handler: Handler): () => void {
  listen();
  handlers.set(id, handler);
  return () => {
    if (handlers.get(id) === handler) handlers.delete(id);
  };
}

/** Calls `watcher` with every report from every terminal, with what was said. */
export function watchAgentHooks(watcher: (report: TerminalAgent) => void): () => void {
  listen();
  watchers.add(watcher);
  return () => watchers.delete(watcher);
}
