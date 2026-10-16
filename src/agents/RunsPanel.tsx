import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { ago, fullDate } from "../ui/time";
import { useWorkspace, type WorkspaceName } from "../workspaces/context";
import { type AgentRun, clearFinished, KIND_NAMES, removeRun, useAgentRuns } from "./runs";
import "./runs.css";

/** Every headless agent run in the workspace, newest first. */
export function RunsPanel() {
  const workspace = useWorkspace();
  const rows = useAgentRuns(workspace);
  const finished = rows.some((r) => r.outcome !== "running");
  const clear: CtxEntry = {
    label: "Clear Finished",
    disabled: !finished,
    onSelect: () => clearFinished(workspace),
  };

  return (
    <div className="runs-panel">
      <div className="runs-head">
        <span className="runs-count">
          {rows.length} {rows.length === 1 ? "run" : "runs"}
        </span>
        <span className="spacer" />
        {finished && (
          <button type="button" className="quiet" onClick={() => clearFinished(workspace)}>
            Clear finished
          </button>
        )}
      </div>
      {rows.length === 0 && <div className="runs-empty">No agent runs in this workspace yet</div>}
      <ul className="runs-list">
        {rows.map((run) => (
          <AgentRow key={run.id} run={run} workspace={workspace} clear={clear} />
        ))}
      </ul>
    </div>
  );
}

interface AgentRowProps {
  run: AgentRun;
  workspace: WorkspaceName;
  /** The menu's Clear Finished. */
  clear: CtxEntry;
}

function AgentRow({ run, workspace, clear }: AgentRowProps) {
  const running = run.outcome === "running";
  const canRetry = !running && run.outcome !== "done" && run.retry;
  const menu = contextMenu(() => [
    running && run.stop && { label: "Stop", onSelect: run.stop },
    canRetry && { label: "Retry", onSelect: canRetry },
    run.open && { label: "Show What It Worked On", onSelect: run.open },
    "-",
    { label: "Remove from List", disabled: running, onSelect: () => removeRun(workspace, run.id) },
    clear,
  ]);
  return (
    <li className="runs-row" onContextMenu={menu}>
      <span className={`runs-dot runs-dot-${run.outcome}`} title={run.outcome} />
      <span className="runs-kind">{KIND_NAMES[run.kind]}</span>
      <span className="runs-subject">{run.subject}</span>
      <span className={`runs-detail${run.outcome === "failed" ? " failed" : ""}`}>
        {run.detail ?? (running ? "starting" : run.outcome)}
      </span>
      <span className="runs-side">
        <span className="runs-actions reveal">
          {running && run.stop && <IconButton icon="stop" label="Stop" onClick={run.stop} />}
          {canRetry && <IconButton icon="retry" label="Retry" onClick={canRetry} />}
          {run.open && <IconButton icon="external" label="Show" onClick={run.open} />}
        </span>
        <span className="runs-time" title={fullDate(run.started)}>
          {ago(run.started)}
        </span>
      </span>
    </li>
  );
}
