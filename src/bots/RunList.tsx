import type { BotRun } from "../bindings";
import { remembered } from "../core/remembered";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { confirmAction } from "../ui/prompt";
import { ago, fullDate } from "../ui/time";
import { allFindings, report } from "./findings";
import { reportIdItems, reportLabel } from "./reportId";
import { forgetRun, useRuns } from "./runs";
import { SevTag } from "./Severity";

/** A report's suggested fixes and how many critical or high findings they leave out, or
 * without them, its findings and how many are high or worse. */
function Outcome({ run }: { run: BotRun }) {
  const r = report(run);
  if (r.minimal) {
    const fixes = r.minimal.fixes?.length ?? 0;
    const off = r.minimal.offTask.length;
    return (
      <>
        Published · {fixes} {fixes === 1 ? "fix" : "fixes"}
        {off > 0 && <span className="sev sev-critical">{off} off-task</span>}
      </>
    );
  }
  const findings = allFindings(r);
  const n = findings.length;
  const serious = (["critical", "high"] as const)
    .map((s) => [s, findings.filter((f) => f.severity === s).length] as const)
    .filter(([, count]) => count > 0);
  return (
    <>
      Published · {n} {n === 1 ? "finding" : "findings"}
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

/** One report, with its actions on hover. */
function RunRow({ run, onOpen, onError }: RowProps) {
  async function remove() {
    if (!(await confirmAction("Delete this report and its findings?", "Delete"))) return;
    const r = await commands.deleteBots(run.id);
    if (r.status === "error") return onError(r.error);
    forgetRun(run.id);
  }

  const del = { label: "Delete Report…", danger: true, onSelect: remove };
  const menu = contextMenu(() => [
    { label: "Open Report", onSelect: () => onOpen(run) },
    ...reportIdItems(run, onError),
    "-",
    del,
  ]);
  return (
    <li onContextMenu={menu}>
      <button type="button" className="bots-run" onClick={() => onOpen(run)}>
        <span className="bots-run-title">
          <Outcome run={run} />
        </span>
        <span className="bots-run-meta">
          <b>{reportLabel(run)}</b> ·{" "}
          <code>
            {run.revset} {run.to.slice(0, 4)}
          </code>{" "}
          · {run.personas.map((p) => p.name).join(", ")}
        </span>
      </button>
      <span className="bots-run-side">
        <span className="bots-run-actions reveal">
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

/** The workspace's reports on the current review, newest first, then its other reports
 * folded. */
export function RunList({ workspace, revset, onOpen, onError }: Props) {
  const runs = useRuns(workspace);
  const open = othersOpen.use();
  const current = runs.filter((r) => r.revset === revset);
  const others = runs.filter((r) => r.revset !== revset);
  const row = (run: BotRun) => <RunRow key={run.id} run={run} onOpen={onOpen} onError={onError} />;

  return (
    <>
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
