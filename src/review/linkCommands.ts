import type { WorkspaceName } from "../core/workspace";
import { prOf } from "../github/pr";
import { commands } from "../ipc";
import { sendBackground } from "../terminal/background";
import { focusTerminal, sendTarget, submitNextSend } from "../terminal/registry";
import { linkText } from "./agentText";
import { currentReview } from "./current";
import { linkAgent, linkedTerminal, notePasted, relinkable, setCurrentThread } from "./linkedAgent";

/** Links the send target's agent to the workspace's PR review, pasting the brief unless it's
 * the terminal linked last. */
export async function startLink(workspace: WorkspaceName, onError: (message: string) => void) {
  if (starting) return;
  starting = true;
  try {
    await link(workspace, onError);
  } finally {
    starting = false;
  }
}

/** A link is being started, so a second press doesn't paste the brief again. */
let starting = false;

async function link(workspace: WorkspaceName, onError: (message: string) => void) {
  const to = sendTarget(workspace);
  if (to && relinkable(workspace, to)) return linkAgent(workspace, to);
  const review = currentReview(workspace);
  const pr = await prOf(workspace);
  if (!review || !pr) return onError("Open the PR's review to link an agent.");
  const rules = await commands.promptText("reply");
  if (rules.status === "error") return onError(rules.error);
  submitNextSend();
  review.send(linkText(pr, rules.data), (terminal) => linkAgent(workspace, terminal));
}

/** Makes thread `id` the one I'm on and focuses the linked terminal, linking one first when
 * none is. */
export function talk(workspace: WorkspaceName, id: number, onError: (message: string) => void) {
  setCurrentThread(workspace, id);
  const linked = linkedTerminal(workspace);
  if (linked) focusTerminal(linked);
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
