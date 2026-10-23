import type { TurnDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import type { Cell } from "../revisions/graphCells";
import { CELL_W } from "../revisions/LogGraph";
import { copy, openMenu } from "../ui/contextMenu";
import { confirmAction } from "../ui/prompt";
import { openTurn } from "./openTurn";
import { reloadTurns, turnName, turnsLabel, turnsOpen, turnTime } from "./turnsStore";
import "./turns.css";

const ROW_H = 24;

/** The toggle on `@`'s second line that shows its turns. */
export function TurnsToggle({ turns }: { turns: TurnDto[] }) {
  const open = turnsOpen.use();
  if (!turns.length) return null;
  return (
    <button
      type="button"
      className="lr-turns"
      aria-expanded={open}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onClick={() => turnsOpen.set(!open)}
    >
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <path d={open ? "M2 3.5l3 3 3-3" : "M3.5 2l3 3-3 3"} />
      </svg>
      {turnsLabel(turns)}
    </button>
  );
}

/** The lines of jj's graph that run on below `@`, for the rows under it. */
function GraphThrough({ below, width }: { below: Cell[]; width: number }) {
  return (
    <svg className="lg-graph" width={width * CELL_W} height={ROW_H} aria-hidden="true">
      {below.map((c, j) =>
        c.d ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: cells have no identity
          <path key={j} d={`M${j * CELL_W + CELL_W / 2} 0V${ROW_H}`} />
        ) : null,
      )}
    </svg>
  );
}

const TERMINAL = "M2.5 4l3 3-3 3M7 10.5h4.5";
const FLAG = "M3.5 12.5V2.5M3.5 3h7l-1.8 2.5L10.5 8h-7";

interface Props {
  workspace: WorkspaceName;
  turns: TurnDto[];
  /** The id of the turn the review shows, if it shows one. */
  shown?: number;
  /** `@`'s last graph line. */
  below: Cell[];
  width: number;
  onError(message: string): void;
}

/** One row per turn and checkpoint under `@`, newest first, while they're open. */
export function TurnRows({ workspace, turns, shown, below, width, onError }: Props) {
  const open = turnsOpen.use();
  if (!open) return null;

  async function undo(t: TurnDto) {
    const ok = await confirmAction(`Undo "${turnName(t)}"?`, "Undo Turn");
    if (!ok) return;
    try {
      await unwrap(commands.undoTurn(workspace, t.id));
    } catch (e) {
      onError(String(e instanceof Error ? e.message : e));
    }
    reloadTurns(workspace);
  }

  async function remove(t: TurnDto) {
    await commands.removeTurn(t.id);
    reloadTurns(workspace);
  }

  function menu(e: React.MouseEvent, t: TurnDto) {
    const turn = t.prompt !== null;
    openMenu(e, [
      { label: "Review", onSelect: () => openTurn(workspace, t) },
      turn && { label: "Undo Turn", disabled: t.running, onSelect: () => undo(t) },
      turn && "-",
      turn && { label: "Copy Prompt", onSelect: () => copy(t.prompt ?? "") },
      "-",
      { label: "Remove", onSelect: () => remove(t) },
    ]);
  }

  return turns.map((t) => (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the log takes the keys
    <div
      key={t.id}
      className={`lt${t.id === shown ? " cursor" : ""}${t.running ? " running" : ""}`}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={() => openTurn(workspace, t)}
      onContextMenu={(e) => menu(e, t)}
    >
      <GraphThrough below={below} width={width} />
      <span className="lt-body">
        <svg
          className={`lt-ic${t.prompt === null ? " ck" : ""}`}
          viewBox="0 0 14 14"
          aria-hidden="true"
        >
          <path d={t.prompt === null ? FLAG : TERMINAL} />
        </svg>
        <span className="lt-p">{turnName(t)}</span>
        <span className="lt-add">+{t.added}</span>
        <span className="lt-del">−{t.removed}</span>
        <span className="lt-t">{turnTime(t)}</span>
      </span>
    </div>
  ));
}
