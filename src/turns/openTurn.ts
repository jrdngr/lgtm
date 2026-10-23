import type { TurnDto, View } from "../bindings";
import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { isIdle } from "../review/idle";
import { setStartView, setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";

/** Reviews `@` in the turn's view: in the review pane as it is when it already holds the
 * turn's change, else on `@`, opening a review tab when there's no pane. */
export function openTurn(workspace: WorkspaceName, turn: TurnDto) {
  const view: View = { kind: "turn", turn: turn.id };
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
