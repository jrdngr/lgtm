import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { confirmAction } from "../ui/prompt";
import { ago, fullDate } from "../ui/time";
import { remembered } from "../workbench/remembered";
import type { WorkspaceName } from "../workspaces/context";
import { finalFindings, report } from "./findings";
import { published, runDetail, runState } from "./personas";
import { type Act, RunControls } from "./RunControls";
import { reportIdItems, reportLabel } from "./reportId";
import { forgetRun, runAction, useRuns } from "./runs";
import { SevTag } from "./Severity";

/** A run's outcome: its state, or once done, its suggested fixes and how many critical or high
 * findings they leave out, or without them, its findings and how many are high or worse. */
function Outcome({ run }: { run: BotRun }) {
  const { state, label } = runState(run);
  if (state !== "done") return <>{label}</>;
  const r = report(run);
  const verb = published(run) ? "Published" : "Done";
  if (r.minimal) {
    const fixes = r.minimal.fixes?.length ?? 0;
    const off = r.minimal.offTask.length;
    return (
      <>
        {verb} · {fixes} {fixes === 1 ? "fix" : "fixes"}
        {off > 0 && <span className="sev sev-critical">{off} off-task</span>}
      </>
    );
  }
  const findings = finalFindings(r);
  const n = findings.length;
  const serious = (["critical", "high"] as const)
    .map((s) => [s, findings.filter((f) => f.severity === s).length] as const)
    .filter(([, count]) => count > 0);
  return (
    <>
      {verb} · {n} {n === 1 ? "finding" : "findings"}
      {serious.map(([s, count]) => (
        <SevTag key={s} severity={s} count={count} />
      ))}
    </>
  );
}

/** Whether reports on other reviews are shown. */
const othersOpen = remembered("lgtm:otherRuns", false, [true, false]);

interface RowProps {
  run: BotRun;
  onOpen(run: BotRun): void;
  onError(message: string | undefined): void;
}

/** One run, with Stop or Retry and Delete on hover. */
function RunRow({ run, onOpen, onError }: RowProps) {
  const { state } = runState(run);
  const act = (f: Parameters<Act>[0]) => runAction(run, f, onError);

  async function remove() {
    if (!(await confirmAction("Delete this report and its findings?", "Delete"))) return;
    const r = await commands.deleteBots(run.id);
    if (r.status === "error") return onError(r.error);
    forgetRun(run.id);
  }

  // The backend won't delete a run while it or one of its fixes runs.
  const del = !run.active &&
    !run.fixes?.some((f) => f.status === "running") && {
      label: "Delete Report…",
      danger: true,
      onSelect: remove,
    };
  const menu = contextMenu(() => [
    { label: "Open Report", onSelect: () => onOpen(run) },
    ...reportIdItems(run, onError),
    {
      label: "Stop",
      disabled: !run.active,
      onSelect: () => act(() => commands.stopBots(run.id)),
    },
    state === "halted" && {
      label: "Retry",
      onSelect: () => act(() => commands.continueBots(run.id, run.options)),
    },
    "-",
    del,
  ]);
  return (
    <li onContextMenu={menu}>
      <button type="button" className="bots-run" onClick={() => onOpen(run)}>
        <span className={`bots-dot bots-dot-${state}`} />
        <span className="bots-run-title">
          <Outcome run={run} />
        </span>
        <span className="bots-run-meta">
          <b>{reportLabel(run)}</b> ·{" "}
          <code>
            {run.revset} {run.to.slice(0, 4)}
          </code>{" "}
          · {runDetail(run)}
        </span>
      </button>
      <span className="bots-run-side">
        <span className="bots-run-actions reveal">
          <RunControls run={run} act={act} />
          <Menu label="Run actions" items={[...reportIdItems(run, onError), del]} />
        </span>
        {run.createdAt !== null && (
          <span className="bots-run-time" title={fullDate(run.createdAt)}>
            {ago(run.createdAt)}
          </span>
        )}
      </span>
    </li>
  );
}

interface Props {
  workspace: WorkspaceName;
  /** The revset of the review being looked at. */
  revset: string | undefined;
  onOpen(run: BotRun): void;
  onError(message: string | undefined): void;
}

/** The workspace's runs on the current review, newest first, then its other runs folded. */
export function RunList({ workspace, revset, onOpen, onError }: Props) {
  const runs = useRuns(workspace);
  const open = othersOpen.use();
  const current = runs.filter((r) => r.revset === revset);
  const others = runs.filter((r) => r.revset !== revset);
  const row = (run: BotRun) => <RunRow key={run.id} run={run} onOpen={onOpen} onError={onError} />;

  return (
    <>
      <div className="bots-section">
        <span className="bots-label">Reports</span>
      </div>
      {current.length === 0 && <div className="bots-empty">No reports yet</div>}
      <ul className="bots-runs">{current.map(row)}</ul>
      {others.length > 0 && (
        <>
          <div className="bots-section">
            <button
              type="button"
              className="bots-label bots-fold"
              aria-expanded={open}
              onClick={() => othersOpen.set(!open)}
            >
              {open ? "▾" : "▸"} Old Reports · {others.length}
            </button>
          </div>
          {open && <ul className="bots-runs">{others.map(row)}</ul>}
        </>
      )}
    </>
  );
}
