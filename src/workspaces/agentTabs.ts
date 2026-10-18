import { bottomDockOf, dockOf, dockReady, revealPanel } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";
import type { AgentInfo } from "./activity";

/** What the agent list calls `agent`: a terminal by its tab's title. */
export function agentName(workspace: WorkspaceName, agent: AgentInfo) {
  if (!agent.tab) return agent.label;
  const panel =
    dockOf(workspace)?.getPanel(agent.tab) ?? bottomDockOf(workspace)?.getPanel(agent.tab);
  return panel?.title ?? agent.label;
}

/** Brings up the tab `agent` runs in, once the workspace's dock is up. */
export async function openAgent(workspace: WorkspaceName, agent: AgentInfo) {
  await dockReady(workspace);
  if (agent.tab) revealPanel(workspace, agent.tab);
}
