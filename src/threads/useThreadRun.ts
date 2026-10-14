import { Channel } from "@tauri-apps/api/core";
import { useState } from "react";
import { endFrom, startRun } from "../agents/runs";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";

export type RunKind = "triage" | "fix";

/** A triage or fix run over the PR's threads, with what the agent is doing now. */
export function useThreadRun(review: CurrentReview | undefined, onError: (e: string) => void) {
  const workspace = useWorkspace();
  const [run, setRun] = useState<{ kind: RunKind; activity: string }>();

  async function start(kind: RunKind) {
    if (!review || run) return;
    setRun({ kind, activity: "" });
    const id = review.id;
    const handle = startRun(workspace, {
      kind,
      subject: kind === "triage" ? "PR threads" : "Threads to fix",
      stop: () => commands.stopThreadRun(id),
      retry: () => start(kind),
    });
    const channel = new Channel<string>();
    channel.onmessage = (activity) => {
      setRun((r) => r && { ...r, activity });
      handle.activity(activity);
    };
    const go = kind === "triage" ? commands.triageThreads : commands.fixThreads;
    const r = await go(id, channel);
    setRun(undefined);
    endFrom(handle, r);
    if (r.status === "error") onError(r.error);
    else review.apply(r.data);
  }

  return {
    run,
    start,
    stop: () => review && commands.stopThreadRun(review.id),
  };
}
