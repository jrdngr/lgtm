import type { TerminalAgent } from "../bindings";
import { commands } from "../ipc";
import { notePasted } from "../review/linkedAgent";
import { watchAgentHooks } from "../terminal/agentHooks";
import {
  interruptAgent,
  promptAgent,
  sendTarget,
  terminalAgentBusy,
  terminalHasAgent,
  terminalTitle,
} from "../terminal/registry";
import type { WorkspaceName } from "../workspaces/context";
import type { Session } from "./model";
import { act, type BusyKind, setBusy, setError } from "./store";

/** How long a pasted prompt has to reach the agent. */
const START_MS = 15_000;
/** How often a step checks its terminal's agent is still there and free. */
const CHECK_MS = 1000;

/** The terminal whose agent runs the session's steps: the one that asked, while its agent is
 * there, else the send target's agent, which takes the session over. */
function home(ws: WorkspaceName, d: Session) {
  if (d.terminal && terminalHasAgent(d.terminal)) return { id: d.terminal, takeover: false };
  const target = sendTarget(ws);
  return target && terminalHasAgent(target) ? { id: target, takeover: true } : undefined;
}

const takeoverText = (d: Session) =>
  `You're taking over lgtm Q&A session ${d.id} ("${d.title}"). Read its prompt, read-first files and questions with qa_questions first.\n\n`;

/** Each session's running step, which Stop ends. */
const stops = new Map<string, () => void>();
/** Each session's last queued step, so its steps run in order. */
const sessionQueues = new Map<string, Promise<unknown>>();
/** Each terminal's last queued step, so one prompt at a time goes to its agent. */
const terminalQueues = new Map<string, Promise<unknown>>();

/** Runs `run` after the one queued last under `key`. */
function queue<T>(queues: Map<string, Promise<unknown>>, key: string, run: () => Promise<T>) {
  const next = (queues.get(key) ?? Promise.resolve()).then(run, run);
  queues.set(key, next);
  return next;
}

export interface StepOptions {
  kind: BusyKind;
  /** The question it's about. */
  about: string;
  prompt: string;
}

/** Runs a step in the session's terminal agent, after the steps queued before it. Resolves
 * with the agent's last message, or undefined when it didn't finish. */
export function runInTerminal(
  ws: WorkspaceName,
  d: Session,
  o: StepOptions,
): Promise<string | undefined> {
  return queue(sessionQueues, d.id, () => step(ws, d, o));
}

/** Stops the session's running step and its agent's turn. */
export const stopStep = (id: string) => stops.get(id)?.();

async function step(ws: WorkspaceName, d: Session, o: StepOptions) {
  const to = home(ws, d);
  if (!to) {
    setError(ws, "Open a terminal running Claude Code to continue this Q&A");
    return undefined;
  }
  if (to.takeover) await act(ws, commands.editQa(ws, d.id, { kind: "terminal", id: to.id }));
  const title = terminalTitle(to.id) || "the terminal";
  const busy = (activity: string) =>
    setBusy(ws, d.id, { kind: o.kind, about: o.about, terminal: to.id, activity });
  busy(`Waiting for ${title}`);
  // Stop works while it waits its turn too; prompt() takes Stop over once it runs.
  let stopped = false;
  let waiting = () => {};
  const cancelled = new Promise<undefined>((resolve) => {
    waiting = () => {
      stopped = true;
      resolve(undefined);
    };
  });
  stops.set(d.id, waiting);
  const run = () => (stopped ? Promise.resolve(undefined) : prompt(d, o, to, title, busy));
  try {
    return await Promise.race([cancelled, queue(terminalQueues, to.id, run)]);
  } catch (e) {
    setError(ws, e instanceof Error ? e.message : String(e));
    return undefined;
  } finally {
    if (stops.get(d.id) === waiting) stops.delete(d.id);
    setBusy(ws, d.id, undefined);
  }
}

/** Prompts the agent in terminal `to` once it's free, and waits for its reply. */
function prompt(
  d: Session,
  o: StepOptions,
  to: { id: string; takeover: boolean },
  title: string,
  busy: (activity: string) => void,
) {
  if (!terminalHasAgent(to.id)) return Promise.reject(new Error(`The agent in ${title} ended`));
  busy("");
  return new Promise<string | undefined>((resolve, reject) => {
    let sent = false;
    let started = false;
    let timer: number | undefined;
    const finish = (reply?: string, error?: string) => {
      unwatch();
      window.clearTimeout(timer);
      window.clearInterval(check);
      stops.delete(d.id);
      if (error) reject(new Error(error));
      else resolve(reply);
    };
    const send = () => {
      sent = true;
      busy("");
      // Its answer belongs to the Q&A, not to a PR thread the terminal is linked to.
      notePasted(to.id, null);
      if (!promptAgent(to.id, (to.takeover ? takeoverText(d) : "") + o.prompt))
        return finish(undefined, `${title} can't take a prompt`);
      timer = window.setTimeout(() => {
        if (!started) finish(undefined, `${title} didn't take the prompt`);
      }, START_MS);
    };
    const unwatch = watchAgentHooks((r: TerminalAgent) => {
      if (r.terminal !== to.id) return;
      if (r.state === "gone") return finish(undefined, `The agent in ${title} ended`);
      if (!sent) {
        if (r.state !== "working" && r.state !== "input") send();
        return;
      }
      if (r.state === "working") {
        started = true;
        busy("");
      } else if (r.state === "input") busy(`Waiting for you in ${title}`);
      else if (started && r.state === "failed") finish(undefined, `The agent in ${title} failed`);
      else if (started) finish(r.reply ?? undefined);
    });
    // A lost hook mustn't hold the terminal's queue: the step ends with its agent or PTY, and
    // starts once the agent is free even if no hook says so.
    const check = window.setInterval(() => {
      if (!terminalHasAgent(to.id)) finish(undefined, `The agent in ${title} ended`);
      else if (!sent && !terminalAgentBusy(to.id)) send();
    }, CHECK_MS);
    stops.set(d.id, () => {
      if (sent) interruptAgent(to.id);
      finish();
    });
    if (terminalAgentBusy(to.id)) busy(`Waiting for ${title}`);
    else send();
  });
}
