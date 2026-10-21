import { useState } from "react";
import type { JjAction } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";
import { reviewAfter } from "../review/RevisionsPanelMenu";
import { loadReview } from "./loadReview";

/** Runs jj actions from the log one at a time, with undo and redo taking back all of the last
 * action's operations. After each, `reload` runs and the review keeps `revset` unless it no
 * longer resolves, or follows `@` after an edit. */
export function useJjActions(
  workspace: WorkspaceName,
  revset: string,
  reload: () => Promise<unknown>,
  onError: (message: string | undefined) => void,
) {
  const [steps, setSteps] = useState(1);
  /** The action running, if one is. */
  const [running, setRunning] = useState<JjAction>();
  /** What the last fetch or push did, which nothing else in the log shows. */
  const [note, setNote] = useState<string>();

  async function act(action: JjAction, _label?: string, follow?: boolean) {
    if (running) return;
    setRunning(action);
    setNote(undefined);
    try {
      onError(undefined);
      const r = await commands.jjAct(workspace, action);
      if (r.status === "error") return onError(r.error);
      if (action.kind !== "undo" && action.kind !== "redo") setSteps(r.data);
      if (action.kind === "fetch") setNote("Fetched");
      if (action.kind === "push") setNote("Pushed");
      await reload();
      await reviewAfter(workspace, (r) => loadReview(workspace, r), revset, follow);
    } finally {
      setRunning(undefined);
    }
  }

  return {
    act,
    busy: !!running,
    running,
    note,
    undo: () => act({ kind: "undo", steps }),
    redo: () => act({ kind: "redo", steps }),
  };
}
