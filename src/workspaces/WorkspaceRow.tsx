import type { WorkspaceDto } from "../bindings";
import { workspaceKey } from "../core/workspace";
import { usePr } from "../github/pr";
import { errorText } from "../ipc";
import { contextMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { AgentGlyph } from "./AgentGlyph";
import { clearFinished, useActivity } from "./activity";
import { agentName, openAgent } from "./agentTabs";
import { COLORS, usePins } from "./pins";
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
      {w.description.split("\n")[0] || <i className="ws-none">no description</i>}
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

interface Props {
  w: WorkspaceDto;
  selected: boolean;
  onSelect(): void;
  onDelete(): void;
  onError(message: string): void;
  /** Starts dragging a pinned row. */
  onPinDrag?(e: React.PointerEvent): void;
  /** Where a dragged pin would land, next to this row. */
  drop?: "before" | "after";
}

/** One workspace: its PR or change, its agents, and a ⋯ menu on hover. */
export function WorkspaceRow({ w, selected, onSelect, onDelete, onError, onPinDrag, drop }: Props) {
  const pr = usePr(w.name);
  const a = useActivity(w.name);
  const key = workspaceKey(w.name);
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
      onDelete,
      onClearDot: () => clearFinished(w.name),
    }),
  );
  return (
    <li
      className={`workspace${selected ? " selected" : ""}${color ? " colored" : ""}${drop ? ` drop-${drop}` : ""}`}
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
        <Lines w={w} onError={onError} />
      </button>
      <span className="workspace-side">
        {w.name !== null && (
          <Menu className="reveal" label="Workspace actions" items={[deleteItem(w, onDelete)]} />
        )}
        <AgentGlyph
          a={a}
          name={(g) => agentName(w.name, g)}
          onOpen={(g) => {
            onSelect();
            openAgent(w.name, g);
          }}
        />
      </span>
    </li>
  );
}
