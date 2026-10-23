import type { TurnDto, View } from "../bindings";
import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { isIdle } from "../review/idle";
import { setStartView, setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";

/** Reviews `@` in the view of a run of turns: in the review pane as it is when it already
 * holds their change, else on `@`, opening a review tab when there's no pane. */
export function openTurns(workspace: WorkspaceName, turns: TurnDto[]) {
  const turn = turns[0];
  if (!turn) return;
  const view: View = { kind: "turn", turns: turns.map((t) => t.id) };
  const target = targetOf(workspace);
  if (target && !isIdle(target)) {
    if (target.changes.some((c) => c.changeId === turn.changeId)) target.view(view);
    else target.load("@", view);
    return;
  }
  setWorkspaceRevset(workspace, "@");
  setStartView(workspace, view);
  const api = dockOf(workspace);
  if (api) openReviewTab(api);
}
