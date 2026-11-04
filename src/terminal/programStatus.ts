import type { IDisposable, Terminal } from "@xterm/xterm";
import type { ActivityState } from "../workspaces/activity";

/** OSC 7501, the program status protocol: a program in the terminal reports its state. */
const OSC = 7501;
const STATES = ["idle", "working", "done", "blocked", "error", "clear"] as const;
type State = (typeof STATES)[number];

/** A report's state, for the root record only, or the query asking whether the terminal reads
 * reports. Anything else is `null`. */
export function parseStatus(body: string): State | "query" | null {
  const pairs = body.split(":").map((p) => p.trim());
  if (pairs[0] === "?") return "query";
  const keys = new Map<string, string>();
  for (const pair of pairs) {
    const at = pair.indexOf("=");
    if (at > 0) keys.set(pair.slice(0, at).trim(), pair.slice(at + 1).trim());
  }
  if (keys.has("id")) return null;
  const state = keys.get("state");
  return STATES.find((s) => s === state) ?? null;
}

/** How a reported state shows as the terminal's agent activity. */
export function activityOf(state: State): ActivityState {
  switch (state) {
    case "working":
      return "running";
    case "blocked":
      return "input";
    case "error":
      return "failed";
    case "clear":
      return "gone";
    default:
      return state;
  }
}

/** Reads the program status reports `term` gets, answering the terminal query with `reply`. */
export function programStatus(
  term: Terminal,
  reply: (data: string) => void,
  onState: (state: ActivityState) => void,
): IDisposable {
  return term.parser.registerOscHandler(OSC, (body) => {
    const status = parseStatus(body);
    if (status === "query") reply(`\x1b]${OSC};?\x1b\\`);
    else if (status) onState(activityOf(status));
    return true;
  });
}
