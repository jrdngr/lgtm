export type { PtyEvent, RepoInfo } from "./bindings";
export { commands } from "./bindings";

export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

/** An error's message, without the "Error: " that `String(e)` adds. */
export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Unwraps a command result, throwing the backend's error message. */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}
