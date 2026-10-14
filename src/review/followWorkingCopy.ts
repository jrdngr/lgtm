import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events, type PrDto } from "../bindings";
import { prOf, setPr } from "../github/pr";
import { commands } from "../ipc";
import { githubOff } from "../tools/agents";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import { setWorkspaceRevset, workspaceRevset } from "./revset";
import { targetOf } from "./target";

/** Workspaces with a follow running, and those moved again while it ran. */
const running = new Set<string>();
const again = new Set<string>();

/** The workspace's PR once its stack PR is in line with the new `@`. */
async function prAfterMove(workspace: WorkspaceName): Promise<PrDto | null> {
  if (githubOff()) return prOf(workspace);
  const r = await commands.syncStackPr(workspace);
  if (r.status === "error") return prOf(workspace);
  setPr(workspace, r.data);
  return r.data;
}

/** Points the workspace's review at the working copy: its PR's view, which ends at `@`, or
 * `@` itself. A review already on it follows `@` when it refreshes. */
async function followOnce(workspace: WorkspaceName) {
  const revset = (await prAfterMove(workspace))?.revset ?? "@";
  if (workspaceRevset(workspace) === revset) return;
  setWorkspaceRevset(workspace, revset);
  await targetOf(workspace)?.load(revset);
}

/** Runs one follow at a time per workspace. Moves that land meanwhile run one more, for
 * wherever `@` is by then. */
async function follow(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  if (running.has(key)) {
    again.add(key);
    return;
  }
  running.add(key);
  try {
    do {
      again.delete(key);
      await followOnce(workspace).catch((e) => console.error("follow @:", e));
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
      .listen((e) => follow(e.payload.workspace));
    return () => {
      sub.then((un) => un());
    };
  }, []);
}
