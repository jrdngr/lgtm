import type { AnswerInput, SessionEdit } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import type { Session } from "./model";
import { act, removeSession, runStep, selectSession, setError, setView } from "./store";

/** Decides question `id`, then has the agent update the questions still open. */
export async function decide(ws: WorkspaceName, d: Session, id: string, answer: AnswerInput) {
  const r = await act(ws, commands.actOnQuestion(ws, d.id, { kind: "decide", id, answer }));
  if (r.status === "error") return false;
  runStep(ws, d.id, { kind: "answer", about: id, subject: `${d.title}: after ${id}` }, () =>
    commands.applyAnswer(ws, d.id, id),
  );
  return true;
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

/** Has the agent build the session's questions from its prompt. */
export function ask(ws: WorkspaceName, id: string, title: string) {
  setView(ws, "questions");
  return runStep(ws, id, { kind: "ask", subject: title }, (ch) => commands.askQa(ws, id, ch));
}

export function writePlan(ws: WorkspaceName, d: Session) {
  return runStep(ws, d.id, { kind: "plan", subject: `${d.title}: plan` }, (ch) =>
    commands.writePlan(ws, d.id, ch),
  );
}

export function apply(ws: WorkspaceName, d: Session) {
  return runStep(ws, d.id, { kind: "apply", subject: `${d.title}: apply` }, (ch) =>
    commands.applyQa(ws, d.id, ch),
  );
}

export const edit = (ws: WorkspaceName, d: Session, e: SessionEdit) =>
  act(ws, commands.editQa(ws, d.id, e));

export async function remove(ws: WorkspaceName, d: Session) {
  const r = await commands.deleteQa(ws, d.id);
  if (r.status === "error") setError(ws, r.error);
  else removeSession(ws, d.id);
}
