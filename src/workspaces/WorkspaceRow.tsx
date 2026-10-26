import type { WorkspaceDto } from "../bindings";
import { workspaceKey } from "../core/workspace";
import { usePr } from "../github/pr";
import { useRoles } from "../terminal/registry";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { AgentGlyph, Moon } from "./AgentGlyph";
import { clearFinished, useActivity } from "./activity";
import { agentName, openAgent } from "./agentTabs";
import { COLORS, usePins } from "./pins";
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
      {w.description.split("\n")[0] || <i className="ws-none">no description</i>}
    </>
  );
}

/** A PR workspace's PR, as its row shows it: unset outside a stack's PRs. */
export type RowPr = { number: number; title: string } | null | undefined;

/** A PR workspace is one line, its PR. Others show their name, then their change. */
export function Lines({ w, pr }: { w: WorkspaceDto; pr: RowPr }) {
  if (pr) {
    return (
      <span className="workspace-title">
        <span className="pill pr">#{pr.number}</span>
        <span className="ellipsis">{pr.title}</span>
      </span>
    );
  }
  return (
    <>
      <span className="workspace-title">
        <span className="ellipsis">{w.title}</span>
        {w.name === null && <span className="pill">main</span>}
        {w.missing && <span className="pill amber">not found</span>}
      </span>
      <span className="workspace-desc">
        <Change w={w} />
      </span>
    </>
  );
}

interface Props {
  w: WorkspaceDto;
  selected: boolean;
  onSelect(): void;
  /** Unset when the workspace can't sleep. */
  onSleep?(): void;
  onDelete(): void;
  /** Asks where its files are. */
  onLocate(): void;
  /** Starts dragging a pinned row. */
  onPinDrag?(e: React.PointerEvent): void;
  /** Where a dragged pin would land, next to this row. */
  drop?: "before" | "after";
  /** Under its repo, in a project's tree. */
  nested?: boolean;
}

/** One workspace: its PR or change, its agents, and a ⋯ menu on hover. */
export function WorkspaceRow({
  w,
  selected,
  onSelect,
  onSleep,
  onDelete,
  onLocate,
  onPinDrag,
  drop,
  nested,
}: Props) {
  const pr = usePr(w.name);
  const a = useActivity(w.name);
  const key = workspaceKey(w.name);
  const role = useRoles(w.name);
  const pins = usePins();
  const color = pins.colors[key];
  const dot = a.state === "done" || a.state === "failed";
  const menu = contextMenu(() =>
    workspaceMenu({
      w,
      pinned: pins.pinned.includes(key),
      color,
      pr,
      dot,
      open: onSelect,
      onSleep,
      onDelete,
      onLocate,
      onClearDot: () => clearFinished(w.name),
    }),
  );
  return (
    <li
      className={`workspace${nested ? " nested" : ""}${selected ? " selected" : ""}${w.asleep ? " asleep" : ""}${color ? " colored" : ""}${drop ? ` drop-${drop}` : ""}`}
      style={color ? ({ "--ws-color": COLORS[color] } as React.CSSProperties) : undefined}
      data-pin={onPinDrag ? key : undefined}
      onPointerDown={onPinDrag}
      onContextMenu={menu}
    >
      <button
        type="button"
        className="workspace-open"
        onClick={onSelect}
        title={w.description.trim() || undefined}
      >
        <Lines w={w} pr={pr && !pr.stack ? pr : null} />
      </button>
      <span className="workspace-side">
        {w.name !== null && (
          <Menu className="reveal" label="Workspace actions" items={[deleteItem(w, onDelete)]} />
        )}
        {w.asleep ? (
          <Moon />
        ) : (
          <AgentGlyph
            a={a}
            name={(g) => agentName(w.name, g)}
            role={(g) => (g.tab ? role(g.tab) : undefined)}
            onOpen={(g) => {
              onSelect();
              openAgent(w.name, g);
            }}
          />
        )}
      </span>
    </li>
  );
}
