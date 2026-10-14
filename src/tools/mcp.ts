import { useSyncExternalStore } from "react";
import type { McpStatus } from "../bindings";
import { commands, unwrap } from "../ipc";

/** Claude Code's registration of lgtm's MCP server, shared by everything that shows it. */
let status: McpStatus | undefined;
const listeners = new Set<() => void>();

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The last status read, `undefined` until one is. */
export function useMcpStatus() {
  return useSyncExternalStore(subscribe, () => status);
}

/** Reads the status again and tells every reader. Rejects with the read's error. */
export async function refreshMcpStatus() {
  status = await unwrap(commands.mcpStatus());
  for (const l of listeners) l();
}
