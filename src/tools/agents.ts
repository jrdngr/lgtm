import { flag } from "./flag";

/** Whether `claude` is missing or too old and the user chose to go on without it. Agent
 * features (persona reviews, comment workshopping) are off then. */
const agents = flag();

export const setAgentsOff = agents.set;
export const agentsOff = agents.get;
export const useAgentsOff = agents.use;

/** Whether `gh` is missing or too old. GitHub features (PR workspaces, publishing) are off. */
const github = flag();

export const setGithubOff = github.set;
export const useGithubOff = github.use;
