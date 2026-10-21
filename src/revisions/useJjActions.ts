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
  const [busy, setBusy] = useState(false);

  async function act(action: JjAction, _label: string, follow?: boolean) {
    if (busy) return;
    setBusy(true);
    try {
      onError(undefined);
      const r = await commands.jjAct(workspace, action);
      if (r.status === "error") return onError(r.error);
      if (action.kind !== "undo" && action.kind !== "redo") setSteps(r.data);
      await reload();
      await reviewAfter(workspace, (r) => loadReview(workspace, r), revset, follow);
    } finally {
      setBusy(false);
    }
  }

  return {
    act,
    busy,
    undo: () => act({ kind: "undo", steps }, "Undid the last action"),
    redo: () => act({ kind: "redo", steps }, "Redid the action"),
  };
}
