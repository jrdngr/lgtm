import { Channel } from "@tauri-apps/api/core";
import { useState } from "react";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";

export type RunKind = "triage" | "fix";

/** A triage or fix run over the PR's threads, with what the agent is doing now. */
export function useThreadRun(review: CurrentReview | undefined, onError: (e: string) => void) {
  const [run, setRun] = useState<{ kind: RunKind; activity: string }>();

  async function start(kind: RunKind) {
    if (!review || run) return;
    setRun({ kind, activity: "" });
    const channel = new Channel<string>();
    channel.onmessage = (activity) => setRun((r) => r && { ...r, activity });
    const go = kind === "triage" ? commands.triageThreads : commands.fixThreads;
    const r = await go(review.id, channel);
    setRun(undefined);
    if (r.status === "error") onError(r.error);
    else review.apply(r.data);
  }

  return {
    run,
    start,
    stop: () => review && commands.stopThreadRun(review.id),
  };
}
