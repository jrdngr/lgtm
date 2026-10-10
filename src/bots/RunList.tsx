import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { runState } from "./personas";
import { forgetRun, refreshRun, useRuns } from "./runs";

function when(ms: number) {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

interface Props {
  workspace: WorkspaceName;
  onOpen(run: BotRun): void;
  onError(message: string | undefined): void;
}

/** The workspace's persona reviews, newest first, with Stop, Retry and Delete. */
export function RunList({ workspace, onOpen, onError }: Props) {
  const runs = useRuns(workspace);

  async function act(run: BotRun, f: () => Promise<{ status: string; error?: string }>) {
    const r = await f();
    onError(r.status === "error" ? r.error : undefined);
    await refreshRun(run.id);
  }

  async function remove(run: BotRun) {
    const r = await commands.deleteBots(run.id);
    if (r.status === "error") return onError(r.error);
    forgetRun(run.id);
  }

  return (
    <>
      <div className="bots-section">Runs</div>
      {runs.length === 0 && <div className="bots-empty">No reviewer runs yet</div>}
      <ul className="bots-runs">
        {runs.map((run) => {
          const { state, label } = runState(run);
          const n = run.personas.length;
          return (
            <li key={run.id}>
              <button type="button" className="bots-run" onClick={() => onOpen(run)}>
                <span>
                  <code>{run.revset}</code> @ <code>{run.to.slice(0, 8)}</code>
                </span>
                <span className="bots-run-meta">
                  <span className={`bots-state bots-state-${state}`}>{label}</span>
                  {n} {n === 1 ? "persona" : "personas"} · {when(run.createdAt ?? 0)}
                </span>
              </button>
              <span className="bots-run-buttons">
                {run.active ? (
                  <button type="button" onClick={() => act(run, () => commands.stopBots(run.id))}>
                    Stop
                  </button>
                ) : (
                  <>
                    {state === "halted" && (
                      <button
                        type="button"
                        onClick={() => act(run, () => commands.continueBots(run.id, run.options))}
                      >
                        Retry
                      </button>
                    )}
                    <button type="button" onClick={() => remove(run)} title="Delete this run">
                      ✕
                    </button>
                  </>
                )}
              </span>
            </li>
          );
        })}
      </ul>
    </>
  );
}
