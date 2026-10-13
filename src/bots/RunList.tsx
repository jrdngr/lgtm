import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { Menu } from "../ui/Menu";
import { ago, fullDate } from "../ui/time";
import type { WorkspaceName } from "../workspaces/context";
import { finalFindings, report } from "./findings";
import { runDetail, runState } from "./personas";
import { RunControls } from "./RunControls";
import { forgetRun, runAction, useRuns } from "./runs";
import { SevTag } from "./Severity";

/** A run's outcome: its state, or its findings once done, with how many are high or worse. */
function Outcome({ run }: { run: BotRun }) {
  const { state, label } = runState(run);
  if (state !== "done") return <>{label}</>;
  const findings = finalFindings(report(run));
  const n = findings.length;
  const serious = (["critical", "high"] as const)
    .map((s) => [s, findings.filter((f) => f.severity === s).length] as const)
    .filter(([, count]) => count > 0);
  return (
    <>
      {n} {n === 1 ? "finding" : "findings"}
      {serious.map(([s, count]) => (
        <SevTag key={s} severity={s} count={count} />
      ))}
    </>
  );
}

interface Props {
  workspace: WorkspaceName;
  onOpen(run: BotRun): void;
  onError(message: string | undefined): void;
}

/** The workspace's persona reviews, newest first, with Stop or Retry and Delete on hover. */
export function RunList({ workspace, onOpen, onError }: Props) {
  const runs = useRuns(workspace);

  async function remove(run: BotRun) {
    const r = await commands.deleteBots(run.id);
    if (r.status === "error") return onError(r.error);
    forgetRun(run.id);
  }

  return (
    <>
      <div className="bots-section">
        <span className="bots-label">Runs</span>
      </div>
      {runs.length === 0 && <div className="bots-empty">No reviewer runs yet</div>}
      <ul className="bots-runs">
        {runs.map((run) => {
          const { state } = runState(run);
          return (
            <li key={run.id}>
              <button type="button" className="bots-run" onClick={() => onOpen(run)}>
                <span className={`bots-dot bots-dot-${state}`} />
                <span className="bots-run-title">
                  <Outcome run={run} />
                </span>
                <span className="bots-run-meta">
                  <code>
                    {run.revset} {run.to.slice(0, 4)}
                  </code>{" "}
                  · {runDetail(run)}
                </span>
              </button>
              <span className="bots-run-side">
                <span className="bots-run-actions reveal">
                  <RunControls run={run} act={(f) => runAction(run, f, onError)} />
                  <Menu
                    label="Run actions"
                    items={[
                      !run.active && { label: "Delete", danger: true, onSelect: () => remove(run) },
                    ]}
                  />
                </span>
                {run.createdAt !== null && (
                  <span className="bots-run-time" title={fullDate(run.createdAt)}>
                    {ago(run.createdAt)}
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>
    </>
  );
}
