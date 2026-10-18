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
/** Each session's last queued step, so steps run one at a time. */
const queues = new Map<string, Promise<unknown>>();

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
  const run = () => step(ws, d, o);
  const next = (queues.get(d.id) ?? Promise.resolve()).then(run, run);
  queues.set(d.id, next);
  return next;
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
  busy("");
  try {
    return await new Promise<string | undefined>((resolve, reject) => {
      let sent = false;
      let started = false;
      let timer: number | undefined;
      const finish = (reply?: string, error?: string) => {
        unwatch();
        window.clearTimeout(timer);
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
      stops.set(d.id, () => {
        if (sent) interruptAgent(to.id);
        finish();
      });
      if (terminalAgentBusy(to.id)) busy(`Waiting for ${title}`);
      else send();
    });
  } catch (e) {
    setError(ws, e instanceof Error ? e.message : String(e));
    return undefined;
  } finally {
    setBusy(ws, d.id, undefined);
  }
}
