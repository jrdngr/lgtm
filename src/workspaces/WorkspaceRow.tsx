import type { ProgressDto, WorkspaceDto } from "../bindings";
import { usePr } from "../github/pr";
import { errorText } from "../ipc";
import { useMarkSetup } from "../review/marks";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { Ring } from "../ui/Ring";
import { clearFinished, useActivity } from "./activity";
import { doneShare, progressTitle } from "./progress";
import { clearStackPr } from "./stack";
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

/** A PR workspace is one line, its PR. Others show their name, with the PR picked from their
 * stack and its ✕, then their change. */
function Lines({ w, onError }: { w: WorkspaceDto; onError(message: string): void }) {
  const pr = usePr(w.name);
  if (pr && !pr.stack) {
    return (
      <span className="workspace-title">
        <span className="pill purple">#{pr.number}</span>
        <span className="ellipsis">{pr.title}</span>
      </span>
    );
  }
  return (
    <>
      <span className="workspace-title">
        <span className="ellipsis">{w.title}</span>
        {w.name === null && <span className="pill">main</span>}
        {pr?.off && <span className="pill amber">Off stack</span>}
        {pr && (
          <span className="pill purple" title={pr.title}>
            #{pr.number}
            {/* biome-ignore lint/a11y/useKeyWithClickEvents: the PR tab's menu deselects by keyboard */}
            <span
              className="pill-x"
              title="Deselect PR"
              onClick={(e) => {
                e.stopPropagation();
                clearStackPr(w.name, pr).catch((err) => onError(errorText(err)));
              }}
            >
              ✕
            </span>
          </span>
        )}
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
  onError(message: string): void;
}

/** One workspace: its PR or change, its agents, review progress, and a ⋯ menu on hover. */
export function WorkspaceRow({ w, progress: p, selected, onSelect, onDelete, onError }: Props) {
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
        <Lines w={w} onError={onError} />
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
