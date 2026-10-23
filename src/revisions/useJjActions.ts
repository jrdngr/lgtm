import { useRef, useState } from "react";
import type { JjAction } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { reviewAfter } from "../review/RevisionsPanelMenu";
import { confirmAction } from "../ui/prompt";
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
  /** Whether an action has started, including asking for a push's plan. */
  const inFlight = useRef(false);
  /** What the last fetch or push did, which nothing else in the log shows. */
  const [note, setNote] = useState<string>();

  /** A push names the bookmarks it would create on the remote, once they're confirmed. Null
   * when cancelled. */
  async function withNewBookmarks(action: JjAction): Promise<JjAction | null> {
    if (action.kind !== "push") return action;
    const plan = await unwrap(commands.pushPlan(workspace));
    const names = plan.newBookmarks;
    if (!names.length) return action;
    const list = names.join(", ");
    const ok = await confirmAction(
      `Push new ${names.length === 1 ? "bookmark" : "bookmarks"} ${list} to ${plan.remote}?`,
      "Push",
    );
    return ok
      ? { kind: "push", remote: plan.remote, range: plan.range, newBookmarks: names }
      : null;
  }

  async function act(requested: JjAction, _label?: string, follow?: boolean) {
    // Set at once, since `running` is only set after a push's plan is asked for.
    if (inFlight.current) return;
    inFlight.current = true;
    let action: JjAction | null;
    try {
      action = await withNewBookmarks(requested);
    } catch (e) {
      inFlight.current = false;
      return onError(String(e));
    }
    if (!action) {
      inFlight.current = false;
      return;
    }
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
      inFlight.current = false;
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
