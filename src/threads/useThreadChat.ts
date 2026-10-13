import { Channel } from "@tauri-apps/api/core";
import { useState } from "react";
import { endFrom, startRun } from "../agents/runs";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import type { Running } from "../review/useComments";
import { useWorkspace } from "../workspaces/context";
import { showThread } from "./focus";

/** The agent chat on threads: the exchange in flight per thread, streaming its reply. */
export function useThreadChat(review: CurrentReview | undefined, onError: (e: string) => void) {
  const workspace = useWorkspace();
  const [running, setRunning] = useState<ReadonlyMap<number, Running>>(new Map());

  const update = (id: number, f: (r: Running) => Running | undefined) =>
    setRunning((m) => {
      const next = new Map(m);
      const r = m.get(id);
      const v = r && f(r);
      if (v) next.set(id, v);
      else next.delete(id);
      return next;
    });

  /** Asks about thread `id`, or with `change`, has the agent edit the code. `draft` is the
   * reply as it stands. Resolves to whether it worked. */
  async function ask(
    id: number,
    message: string,
    draft: string,
    change: boolean,
  ): Promise<boolean> {
    if (!review || running.has(id)) return false;
    const t = review.threads.find((x) => x.id === id);
    const handle = startRun(workspace, {
      kind: change ? "change" : "reply",
      subject: t ? threadSubject(t.path, t.line) : "Thread",
      stop: () => commands.stopWorkshop(id),
      retry: () => ask(id, message, draft, change),
      open: () => showThread(workspace, id),
    });
    setRunning((m) => new Map(m).set(id, { ask: message, change, reply: "" }));
    const channel = new Channel<string>();
    channel.onmessage = (text) => update(id, (r) => ({ ...r, reply: r.reply + text }));
    const r = await commands.workshopComment(review.id, id, { message, draft, change }, channel);
    update(id, () => undefined);
    endFrom(handle, r);
    if (r.status === "error") {
      onError(r.error);
      return false;
    }
    review.apply(r.data);
    return true;
  }

  return { running, ask, stop: (id: number) => commands.stopWorkshop(id) };
}

/** Where a thread is, for a run's subject. PR-level threads have no path. */
export const threadSubject = (path: string, line: number | null) =>
  path ? `${path}${line ? `:${line}` : ""}` : "PR comment";

export type ThreadChat = ReturnType<typeof useThreadChat>;
