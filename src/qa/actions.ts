import type { AnswerInput, SessionEdit } from "../bindings";
import { commands } from "../ipc";
import { remembered } from "../workbench/remembered";
import type { WorkspaceName } from "../workspaces/context";
import { dependents, type Session } from "./model";
import { act, removeSession, runStep, selectSession, setError, setView } from "./store";

/** Decides question `id`, then has the agent update the questions still open. */
export async function decide(ws: WorkspaceName, d: Session, id: string, answer: AnswerInput) {
  const r = await act(ws, commands.actOnQuestion(ws, d.id, { kind: "decide", id, answer }));
  if (r.status === "error") return false;
  updateAfter(ws, d, id);
  return true;
}

/** Each session's last queued update, so updates run one at a time in one conversation. */
const updates = new Map<string, Promise<unknown>>();

/** Has the agent update the questions that depend on `id` after it was decided. Nothing runs
 * when none do. Updates queue behind the one running. */
export function updateAfter(ws: WorkspaceName, d: Session, id: string) {
  if (!dependents(d, id).length) return;
  const run = () =>
    runStep(ws, d.id, { kind: "answer", about: id, subject: `${d.title}: after ${id}` }, () =>
      commands.applyAnswer(ws, d.id, id),
    );
  const next = (updates.get(d.id) ?? Promise.resolve()).then(run, run);
  updates.set(d.id, next);
  return next;
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
  const r = await runStep(
    ws,
    d.id,
    { kind: "discuss", about: id, subject: `${d.title}: ${id}`, streams: true },
    (ch) => commands.discussQuestion(ws, d.id, id, message, ch),
  );
  const decided = wasOpen && r.status === "ok" && state(r.data) === "decided";
  if (decided) updateAfter(ws, r.data, id);
  return decided;
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

/** Has the agent build the session's questions from its prompt. */
export function ask(ws: WorkspaceName, id: string, title: string) {
  setView(ws, "questions");
  const auto = autoAnswer.get();
  return runStep(ws, id, { kind: "ask", subject: title }, (ch) => commands.askQa(ws, id, auto, ch));
}

export const archiveLabel = (d: Session) => (d.archived ? "Unarchive" : "Archive");

export const edit = (ws: WorkspaceName, d: Session, e: SessionEdit) =>
  act(ws, commands.editQa(ws, d.id, e));

export async function remove(ws: WorkspaceName, d: Session) {
  const r = await commands.deleteQa(ws, d.id);
  if (r.status === "error") setError(ws, r.error);
  else removeSession(ws, d.id);
}
