import type { AnswerInput, SessionEdit } from "../bindings";
import { commands } from "../ipc";
import { remembered } from "../workbench/remembered";
import type { WorkspaceName } from "../workspaces/context";
import { dependents, type Session } from "./model";
import { act, getQa, loadSessions, removeSession, selectSession, setError } from "./store";
import { runInTerminal } from "./terminalSteps";

/** Decides question `id`, then has the agent update the questions still open. */
export async function decide(ws: WorkspaceName, d: Session, id: string, answer: AnswerInput) {
  const r = await act(ws, commands.actOnQuestion(ws, d.id, { kind: "decide", id, answer }));
  if (r.status === "error") return false;
  updateAfter(ws, d, id);
  return true;
}

const about = (d: Session) => `lgtm Q&A "${d.title}" (session ${d.id})`;

/** Has the agent update the questions that depend on `id` after it was decided. Nothing runs
 * when none do. */
export function updateAfter(ws: WorkspaceName, d: Session, id: string) {
  if (!dependents(d, id).length) return;
  return runInTerminal(ws, d, {
    kind: "answer",
    about: id,
    prompt: `${about(d)}: I answered ${id}. Read what depends on it with qa_dependents and apply it with qa_update.`,
  });
}

/** Asks the agent to explain option `option` of question `id`, in its discussion. */
export function explain(ws: WorkspaceName, d: Session, id: string, option: number) {
  const q = d.queue.questions.find((x) => x.id === id);
  const label = q?.text.options[option]?.label ?? "";
  return discuss(
    ws,
    d,
    id,
    `Explain option ${option + 1} (${label}) in more detail: what it means here, how the context bears on it, and what it costs. Don't decide anything.`,
  );
}

/** A discussion turn on question `id`. When the user settles it there, the agent decides it
 * and then updates the questions still open. Returns whether it was decided. */
export async function discuss(ws: WorkspaceName, d: Session, id: string, message: string) {
  const state = (s: Session) => s.queue.questions.find((q) => q.id === id)?.state;
  const wasOpen = state(d) === "open";
  const mine = await act(ws, commands.addQaTurn(ws, d.id, id, true, message));
  if (mine.status === "error") return false;
  const reply = await runInTerminal(ws, d, {
    kind: "discuss",
    about: id,
    prompt: `${about(d)}, about ${id}: ${message}\n\n(qa_question has ${id}, our discussion and how we discuss it.)`,
  });
  if (reply?.trim()) await act(ws, commands.addQaTurn(ws, d.id, id, false, reply.trim()));
  else await loadSessions(ws);
  const now = getQa(ws).sessions.find((x) => x.id === d.id);
  const decided = wasOpen && !!now && state(now) === "decided";
  if (decided && now) updateAfter(ws, now, id);
  return decided;
}

/** Has the agent rewrite question `id` with what came out of its discussion. */
export function reask(ws: WorkspaceName, d: Session, id: string) {
  return runInTerminal(ws, d, {
    kind: "reask",
    about: id,
    prompt: `${about(d)}: rewrite ${id} with what came out of our discussion, with qa_rewrite. qa_question has it and the discussion.`,
  });
}

/** Saves a prompt as a new session, or into session `d`. Returns the session's id. */
export async function savePrompt(
  ws: WorkspaceName,
  d: Session | undefined,
  prompt: string,
  attach: string[],
) {
  if (d && d.prompt === prompt && d.attach.join("\n") === attach.join("\n")) return d.id;
  const r = await act(
    ws,
    d
      ? commands.editQa(ws, d.id, { kind: "prompt", prompt, attach })
      : commands.createQa(ws, prompt, attach),
  );
  if (r.status === "error") return undefined;
  selectSession(ws, r.data.id);
  return r.data.id;
}

/** Whether asking has the agent answer the obvious questions itself. */
export const autoAnswer = remembered("lgtm:qa:autoAnswer", true, [true, false]);

export const archiveLabel = (d: Session) => (d.archived ? "Unarchive" : "Archive");

export const edit = (ws: WorkspaceName, d: Session, e: SessionEdit) =>
  act(ws, commands.editQa(ws, d.id, e));

export async function remove(ws: WorkspaceName, d: Session) {
  const r = await commands.deleteQa(ws, d.id);
  if (r.status === "error") setError(ws, r.error);
  else removeSession(ws, d.id);
}
