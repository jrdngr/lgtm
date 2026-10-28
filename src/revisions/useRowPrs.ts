import { useCallback, useEffect, useRef, useState } from "react";
import type { LogCommit, NearbyPrDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useGithubOff } from "../tools/agents";
import { useRepoChanged } from "../workspaces/useRepoChanged";

/** An open PR, pilled on its head commit. */
export interface RowPr {
  number: number;
  title: string;
  url: string;
  bookmark: string;
  /** The PR the workspace was checked out for, rather than one on its stack. */
  own: boolean;
}

/** The open PRs around the workspace's `@`, and the PR its workspace was checked out for, by
 * the commit each is on, and whether the open PRs are known. `refreshed` changing asks GitHub
 * again. */
export function useRowPrs(workspace: WorkspaceName, refreshed: number) {
  const githubOff = useGithubOff();
  const pr = usePr(workspace);
  const [nearby, setNearby] = useState<NearbyPrDto[]>([]);
  const [known, setKnown] = useState(false);
  const latest = useRef(workspace);
  latest.current = workspace;

  const load = useCallback(
    async (refresh: boolean) => {
      const r = await commands.nearbyPrs(workspace, refresh).catch(() => undefined);
      if (latest.current !== workspace) return;
      // Listing PRs is a convenience: without gh or a GitHub remote there are no pills.
      setNearby(r?.status === "ok" ? r.data.prs : []);
      setKnown(r?.status === "ok");
    },
    [workspace],
  );

  useEffect(() => {
    if (githubOff) {
      setNearby([]);
      setKnown(true);
    } else load(refreshed > 0);
  }, [githubOff, load, refreshed]);
  // A background snapshot only rewrites `@`, which moves no bookmarks.
  useRepoChanged(
    () => {
      if (!githubOff) load(false);
    },
    workspace,
    { snapshots: false },
  );

  const prOf = useCallback(
    (c: LogCommit): RowPr | undefined => {
      if (pr && !pr.stack && c.commitId === pr.head) {
        return { ...pr, bookmark: pr.headRef, own: true };
      }
      const n = nearby.find((p) => p.change && c.changeId.startsWith(p.change));
      return n && { ...n, own: false };
    },
    [pr, nearby],
  );
  return { prOf, known };
}
