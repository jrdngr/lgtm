import { bottomDockOf, dockOf, dockReady, revealPanel } from "../workbench/docks";
import { isReviewPanel } from "../workbench/panels";
import { type AgentInfo, REVIEW_TAB } from "./activity";
import type { WorkspaceName } from "./context";

/** The id of the panel `agent` runs in, if it's open. */
function panelOf(workspace: WorkspaceName, agent: AgentInfo) {
  if (agent.tab !== REVIEW_TAB) return agent.tab;
  return dockOf(workspace)?.panels.find(isReviewPanel)?.id;
}

/** What the agent list calls `agent`: a terminal by its tab's title. */
export function agentName(workspace: WorkspaceName, agent: AgentInfo) {
  if (agent.headless || !agent.tab) return agent.label;
  const panel =
    dockOf(workspace)?.getPanel(agent.tab) ?? bottomDockOf(workspace)?.getPanel(agent.tab);
  return panel?.title ?? agent.label;
}

/** Brings up the tab `agent` runs in, once the workspace's dock is up. */
export async function openAgent(workspace: WorkspaceName, agent: AgentInfo) {
  await dockReady(workspace);
  const id = panelOf(workspace, agent);
  if (id) revealPanel(workspace, id);
}
