import type { ProgressDto, WorkspaceDto } from "../bindings";
import { usePr } from "../github/pr";
import { Menu } from "../ui/Menu";
import { Ring } from "../ui/Ring";
import { doneShare, progressTitle } from "./progress";

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

interface Props {
  w: WorkspaceDto;
  progress?: ProgressDto;
  selected: boolean;
  onSelect(): void;
  onDelete(): void;
}

/** One workspace: its PR or change, review progress, and a ⋯ menu on hover. */
export function WorkspaceRow({ w, progress: p, selected, onSelect, onDelete }: Props) {
  return (
    <li className={`workspace${selected ? " selected" : ""}`}>
      <button type="button" className="workspace-open" onClick={onSelect} title={w.root}>
        <Lines w={w} />
      </button>
      <span className="workspace-side">
        {w.name !== null && (
          <Menu
            className="reveal"
            label="Workspace actions"
            items={[{ label: "Delete…", danger: true, onSelect: onDelete }]}
          />
        )}
        {p && p.changed > 0 && <Ring done={doneShare(p)} title={progressTitle(p)} percent />}
      </span>
    </li>
  );
}
