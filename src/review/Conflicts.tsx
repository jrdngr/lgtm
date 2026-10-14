import { Channel } from "@tauri-apps/api/core";
import { useState } from "react";
import { startRun } from "../agents/runs";
import type { ChangeDto, ResolveDto } from "../bindings";
import { commands } from "../ipc";
import { useAgentsOff } from "../tools/agents";
import { Busy } from "../ui/Busy";
import type { WorkspaceName } from "../workspaces/context";

interface Run {
  change: string;
  text: string;
  result?: ResolveDto;
  error?: string;
}

interface Props {
  workspace: WorkspaceName;
  changes: ChangeDto[];
  /** Pastes text into one of the workspace's terminals. */
  send(text: string): void;
  /** Called when a resolution finished, to show it. */
  onDone(): void;
}

const terminalPrompt = (change: string) =>
  `Resolve the conflicts in jj commit ${change}: run \`jj new ${change}\`, fix the conflict markers ` +
  "it shows (`jj resolve --list`), then run `jj squash` to move the resolution into it.";

function outcome(run: Run) {
  if (run.error) return run.error;
  if (!run.result) return undefined;
  if (run.result.stopped) return "Stopped. The conflicts are as they were.";
  return run.result.conflicted ? "Done, but it still has conflicts." : "Resolved.";
}

/** Conflicted changes in the review, each of which can go to an agent to resolve. */
export function Conflicts({ workspace, changes, send, onDone }: Props) {
  const agentsOff = useAgentsOff();
  const [run, setRun] = useState<Run>();
  const conflicted = changes.filter((c) => c.conflict);
  const running = run && !run.result && !run.error;

  async function resolve(change: string) {
    setRun({ change, text: "" });
    const handle = startRun(workspace, {
      kind: "resolve",
      subject: change,
      stop: () => commands.stopResolve(workspace),
      retry: () => resolve(change),
    });
    const channel = new Channel<string>();
    channel.onmessage = (t) => setRun((r) => r && { ...r, text: r.text + t });
    const r = await commands.resolveConflict(workspace, change, channel);
    if (r.status === "error") handle.end("failed", r.error);
    else if (r.data.stopped) handle.end("stopped");
    else handle.end("done", r.data.conflicted ? "Still has conflicts" : "Resolved");
    setRun((prev) =>
      r.status === "ok"
        ? { change, text: r.data.text, result: r.data }
        : { change, text: prev?.text ?? "", error: r.error },
    );
    onDone();
  }

  if (conflicted.length === 0 && !run) return null;
  return (
    <>
      {conflicted.map((c) => (
        <div key={c.changeId} className="review-note warn">
          {c.changeId} {c.description || "(no description)"} has conflicts.
          {!agentsOff && (
            <button type="button" disabled={!!running} onClick={() => resolve(c.changeId)}>
              Resolve with agent
            </button>
          )}
          <button type="button" onClick={() => send(terminalPrompt(c.changeId))}>
            Send to terminal
          </button>
        </div>
      ))}
      {run && (
        <div className="review-note resolve-run">
          <div>
            {running ? <Busy>Resolving {run.change}…</Busy> : `${run.change}: ${outcome(run)}`}
            {running ? (
              <button type="button" onClick={() => commands.stopResolve(workspace)}>
                Stop
              </button>
            ) : (
              <button type="button" onClick={() => setRun(undefined)}>
                Dismiss
              </button>
            )}
          </div>
          {run.text && <div className="msg-text">{run.text}</div>}
        </div>
      )}
    </>
  );
}
