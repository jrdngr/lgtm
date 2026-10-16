import { flag } from "./flag";

/** Whether `claude` is missing or too old and the user chose to go on without it. Agent
 * features (persona reviews, triage, Q&A) are off then. */
const agents = flag();

export const agentsOff = agents.get;
export const useAgentsOff = agents.use;

/** Whether `gh` is missing or too old. GitHub features (PR workspaces, publishing) are off. */
const github = flag();

export const githubOff = github.get;
export const useGithubOff = github.use;

/** Turns agent and GitHub features off while `claude` or `gh` is among the tools with a
 * problem, and back on once it isn't. */
export function setToolProblems(tools: string[]) {
  agents.set(tools.includes("claude"));
  github.set(tools.includes("gh"));
}
