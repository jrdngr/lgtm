import { useEffect, useRef, useState } from "react";
import type { DiscussionDto, HeadDto, PrDto } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";

/** A PR's conversation and state on GitHub, read when the PR changes and on each sync. */
export function useDiscussion(workspace: WorkspaceName, pr: PrDto) {
  const [d, setD] = useState<DiscussionDto>();
  const [head, setHead] = useState<HeadDto>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [syncedAt, setSyncedAt] = useState<number>();

  async function load() {
    setLoading(true);
    const [r, h] = await Promise.all([
      commands.prDiscussion(workspace),
      commands.prHead(workspace),
    ]);
    setLoading(false);
    if (h.status === "ok") setHead(h.data);
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    setD(r.data);
    setSyncedAt(Date.now());
  }

  // Cleared only for another PR, so a reload of the same one doesn't flash empty.
  const which = `${pr.repo}#${pr.number}`;
  const shown = useRef(which);
  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when the workspace's PR does
  useEffect(() => {
    if (shown.current !== which) {
      shown.current = which;
      setD(undefined);
      setHead(undefined);
    }
    load();
  }, [pr]);

  return { d, head, error, loading, syncedAt, load };
}

export type Discussion = ReturnType<typeof useDiscussion>;
