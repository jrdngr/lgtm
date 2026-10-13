import { useState } from "react";
import type { BotRun } from "../bindings";
import { runDetail, runState } from "../bots/personas";
import { RunControls } from "../bots/RunControls";
import { runAction, useRuns } from "../bots/runs";
import { IconButton } from "../ui/icons";
import { ago, fullDate } from "../ui/time";
import { dockOf } from "../workbench/docks";
import { openReport } from "../workbench/panels";
import { useWorkspace, type WorkspaceName } from "../workspaces/context";
import { type AgentRun, clearFinished, KIND_NAMES, useAgentRuns } from "./runs";
import "./runs.css";

type Row = { at: number; agent: AgentRun } | { at: number; bot: BotRun };

/** Every agent run in the workspace, newest first: chats, triage and fixes, walkthroughs,
 * conflict resolves, Q&A steps, and persona reviews. */
export function RunsPanel() {
  const workspace = useWorkspace();
  const agents = useAgentRuns(workspace);
  const bots = useRuns(workspace);
  const [error, setError] = useState<string>();
  const rows: Row[] = [
    ...agents.map((agent) => ({ at: agent.started, agent })),
    ...bots.map((bot) => ({ at: bot.createdAt ?? 0, bot })),
  ].sort((a, b) => b.at - a.at);
  const finished = agents.some((r) => r.outcome !== "running");

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
      {error && <div className="review-note warn">{error}</div>}
      {rows.length === 0 && <div className="runs-empty">No agent runs in this workspace yet</div>}
      <ul className="runs-list">
        {rows.map((row) =>
          "agent" in row ? (
            <AgentRow key={`a${row.agent.id}`} run={row.agent} />
          ) : (
            <BotRow key={`b${row.bot.id}`} run={row.bot} workspace={workspace} onError={setError} />
          ),
        )}
      </ul>
    </div>
  );
}

function AgentRow({ run }: { run: AgentRun }) {
  const running = run.outcome === "running";
  return (
    <li className="runs-row">
      <span className={`runs-dot runs-dot-${run.outcome}`} title={run.outcome} />
      <span className="runs-kind">{KIND_NAMES[run.kind]}</span>
      <span className="runs-subject">{run.subject}</span>
      <span className={`runs-detail${run.outcome === "failed" ? " failed" : ""}`}>
        {run.detail ?? (running ? "starting" : run.outcome)}
      </span>
      <span className="runs-side">
        <span className="runs-actions reveal">
          {running && run.stop && <IconButton icon="stop" label="Stop" onClick={run.stop} />}
          {!running && run.retry && run.outcome !== "done" && (
            <IconButton icon="retry" label="Retry" onClick={run.retry} />
          )}
          {run.open && <IconButton icon="external" label="Show" onClick={run.open} />}
        </span>
        <span className="runs-time" title={fullDate(run.started)}>
          {ago(run.started)}
        </span>
      </span>
    </li>
  );
}

interface BotRowProps {
  run: BotRun;
  workspace: WorkspaceName;
  onError(message: string | undefined): void;
}

function BotRow({ run, workspace, onError }: BotRowProps) {
  const { state } = runState(run);
  const outcome = state === "halted" ? "failed" : state;
  const open = () => {
    const api = dockOf(workspace);
    if (api) openReport(api, run.id);
  };
  return (
    <li className="runs-row">
      <span className={`runs-dot runs-dot-${outcome}`} title={outcome} />
      <span className="runs-kind">Reviewers</span>
      <span className="runs-subject">{run.revset}</span>
      <span className="runs-detail">{runDetail(run)}</span>
      <span className="runs-side">
        <span className="runs-actions reveal">
          <RunControls run={run} act={(f) => runAction(run, f, onError)} />
          <IconButton icon="external" label="Show" onClick={open} />
        </span>
        {run.createdAt !== null && (
          <span className="runs-time" title={fullDate(run.createdAt)}>
            {ago(run.createdAt)}
          </span>
        )}
      </span>
    </li>
  );
}
