import type { ProgressDto, WorkspaceDto } from "../bindings";
import { usePr } from "../github/pr";
import { useMarkSetup } from "../review/marks";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { Ring } from "../ui/Ring";
import { clearFinished, useActivity } from "./activity";
import { doneShare, progressTitle } from "./progress";
import { deleteItem, workspaceMenu } from "./workspaceMenu";

/** Characters of a change id shown, its unique prefix included. */
const ID_LENGTH = 4;

/** A change id with its unique prefix colored, as jj colors it. */
function ChangeId({ id, unique }: { id: string; unique: number }) {
  return (
    <span className="ws-id">
      <span className="ws-unique">{id.slice(0, unique)}</span>
      {id.slice(unique, Math.max(unique, ID_LENGTH))}
    </span>
  );
}

/** The working copy's change and description. */
function Change({ w }: { w: WorkspaceDto }) {
  return (
    <>
      <ChangeId id={w.changeId} unique={w.unique} />{" "}
      {w.description || <i className="ws-none">no description</i>}
    </>
  );
}

/** Title and second line: a PR workspace leads with its PR, others with their name. */
function Lines({ w }: { w: WorkspaceDto }) {
  const pr = usePr(w.name);
  if (pr) {
    return (
      <>
        <span className="workspace-title">
          <span className="pill purple">#{pr.number}</span>
          <span className="ellipsis">{pr.title}</span>
        </span>
        <span className="workspace-desc">{w.title}</span>
      </>
    );
  }
  return (
    <>
      <span className="workspace-title">
        <span className="ellipsis">{w.title}</span>
        {w.name === null && <span className="pill">main</span>}
      </span>
      <span className="workspace-desc">
        <Change w={w} />
      </span>
    </>
  );
}

/** A spinner while the workspace's agents work, or a dot once one finishes while you're away:
 * green, or red when one failed. */
function ActivityMark({ w }: { w: WorkspaceDto }) {
  const a = useActivity(w.name);
  if (!a) return null;
  if (a.kind === "working") {
    return <span className="ws-activity working" title={`Working: ${a.labels.join(", ")}`} />;
  }
  const tone = a.failed ? "failed" : "done";
  return <span className={`ws-activity ${tone}`} title={`Finished: ${a.labels.join(", ")}`} />;
}

interface Props {
  w: WorkspaceDto;
  progress?: ProgressDto;
  selected: boolean;
  onSelect(): void;
  onDelete(): void;
}

/** One workspace: its PR or change, its agents, review progress, and a ⋯ menu on hover. */
export function WorkspaceRow({ w, progress: p, selected, onSelect, onDelete }: Props) {
  const setup = useMarkSetup();
  const pr = usePr(w.name);
  const dot = useActivity(w.name)?.kind === "finished";
  const menu = contextMenu(() =>
    workspaceMenu({
      w,
      pr,
      dot,
      open: onSelect,
      onDelete,
      onClearDot: () => clearFinished(w.name),
    }),
  );
  return (
    <li className={`workspace${selected ? " selected" : ""}`} onContextMenu={menu}>
      <button type="button" className="workspace-open" onClick={onSelect} title={w.root}>
        <Lines w={w} />
      </button>
      <span className="workspace-side">
        {w.name !== null && (
          <Menu className="reveal" label="Workspace actions" items={[deleteItem(w, onDelete)]} />
        )}
        <ActivityMark w={w} />
        {p && p.changed > 0 && <Ring done={doneShare(p)} title={progressTitle(p, setup)} percent />}
      </span>
    </li>
  );
}
