import type { AnswerInput } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import type { Design } from "./model";
import { act, runStep, showDesign } from "./store";

/** Decides question `id`, then has the agent update the questions still open. */
export async function decide(ws: WorkspaceName, d: Design, id: string, answer: AnswerInput) {
  const r = await act(ws, commands.actOnQuestion(ws, d.slug, { kind: "decide", id, answer }));
  if (r.status === "error") return false;
  runStep(
    ws,
    d.slug,
    {
      kind: "answer",
      about: id,
      subject: `${d.name}: after ${id}`,
      open: () => showDesign(ws, d.slug, "questions"),
    },
    () => commands.applyAnswer(ws, d.slug, id),
  );
  return true;
}
