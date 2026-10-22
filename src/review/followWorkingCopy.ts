import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events, type PrDto } from "../bindings";
import { logger } from "../core/log";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { prOf, setPr } from "../github/pr";
import { commands } from "../ipc";
import { githubOff } from "../tools/agents";
import { prFollowsReview } from "../workspaces/stack";
import { setWorkspaceRevset, workspaceRevset } from "./revset";
import { targetOf } from "./target";

const log = logger("follow @");

/** Workspaces with a follow running, and those moved again while it ran. */
const running = new Set<string>();
const again = new Set<string>();

/** The workspace's PR, its stack PR's head kept current. */
async function prAfterMove(workspace: WorkspaceName): Promise<PrDto | null> {
  if (githubOff()) return prOf(workspace);
  const r = await commands.syncStackPr(workspace);
  if (r.status === "error") return prOf(workspace);
  setPr(workspace, r.data);
  return r.data;
}

/** Whether `@` is in the review of `revset` or sits right on it. */
async function holdsAt(workspace: WorkspaceName, revset: string) {
  const r = await commands.workingCopy(workspace, revset);
  return r.status === "ok" && r.data.place !== "outside";
}

/** Keeps a review of the workspace's PR, or one `@` is in or sits right on. Any other goes to
 * the commits of the PR the workspace was checked out for, or to `@`. A review already on it
 * refreshes on its own. */
async function followOnce(workspace: WorkspaceName) {
  const pr = await prAfterMove(workspace);
  const current = workspaceRevset(workspace);
  if (current && (current === pr?.revset || (await holdsAt(workspace, current)))) return;
  const revset = pr && !pr.stack ? pr.revset : "@";
  if (current === revset) return;
  setWorkspaceRevset(workspace, revset);
  await prFollowsReview(workspace, revset);
  await targetOf(workspace)?.load(revset);
}

/** Points the workspace's review at the working copy after `@` moved. Runs one follow at a
 * time per workspace. Moves that land meanwhile run one more, for wherever `@` is by then. */
export async function followWorkingCopy(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  if (running.has(key)) {
    again.add(key);
    return;
  }
  running.add(key);
  try {
    do {
      again.delete(key);
      await followOnce(workspace).catch((e) => log.error(e));
    } while (again.has(key));
  } finally {
    running.delete(key);
  }
}

/** When a terminal or an agent moves a workspace's `@`, its review goes to the working copy. */
export function useFollowWorkingCopy() {
  useEffect(() => {
    const sub = events
      .workingCopyMoved(getCurrentWebviewWindow())
      .listen((e) => followWorkingCopy(e.payload.workspace));
    return () => {
      sub.then((un) => un());
    };
  }, []);
}
