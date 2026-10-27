import type { WorkspaceName } from "../core/workspace";
import { prOf } from "../github/pr";
import { commands } from "../ipc";
import { sendBackground } from "../terminal/background";
import { focusTerminal, sendTarget, submitNextSend } from "../terminal/registry";
import { linkText } from "./agentText";
import { currentReview } from "./current";
import { linkAgent, linkedTerminal, notePasted, setCurrentThread } from "./linkedAgent";

/** Links the send target's agent to the workspace's PR review and focuses its terminal. lgtm's
 * hook gives it the brief with its next prompt. */
export async function startLink(workspace: WorkspaceName, onError: (message: string) => void) {
  if (starting) return;
  starting = true;
  try {
    await link(workspace, onError);
  } finally {
    starting = false;
  }
}

/** A link is being started, so a second press waits for it. */
let starting = false;

async function link(workspace: WorkspaceName, onError: (message: string) => void) {
  const to = sendTarget(workspace);
  if (!to) return onError("Open a terminal to link an agent.");
  const pr = await prOf(workspace);
  if (!currentReview(workspace) || !pr) return onError("Open the PR's review to link an agent.");
  const rules = await commands.promptText("reply");
  if (rules.status === "error") return onError(rules.error);
  linkAgent(workspace, to, linkText(pr, rules.data));
  focusTerminal(to);
}

/** Makes thread `id` the one I'm on and focuses the linked terminal, first linking the
 * interactive terminal when it isn't the one linked. */
export function talk(workspace: WorkspaceName, id: number, onError: (message: string) => void) {
  setCurrentThread(workspace, id);
  const linked = linkedTerminal(workspace);
  const target = sendTarget(workspace);
  if (linked && (!target || linked === target)) focusTerminal(linked);
  else startLink(workspace, onError);
}

/** Pastes `text` about `thread` (or no one thread) into the background terminal and submits
 * it. A thread it's about becomes the one I'm on. */
export async function sendToBackgroundAgent(
  workspace: WorkspaceName,
  text: string,
  thread: number | null,
  onError: (message: string) => void,
) {
  if (thread !== null) setCurrentThread(workspace, thread);
  submitNextSend();
  const sent = await sendBackground(workspace, text);
  if ("terminal" in sent) notePasted(sent.terminal, thread);
  else if (sent.error) onError(sent.error);
}
