import { useEffect, useRef, useState } from "react";
import type { TurnDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import type { Cell } from "../revisions/graphCells";
import { CELL_W } from "../revisions/LogGraph";
import { copy, openMenu } from "../ui/contextMenu";
import { confirmAction } from "../ui/prompt";
import { openTurns } from "./openTurn";
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
const PERSON =
  "M7 6.5a2.25 2.25 0 1 0 0-4.5 2.25 2.25 0 0 0 0 4.5zM2.5 12.5c0-2.6 2-4.2 4.5-4.2s4.5 1.6 4.5 4.2";
const ICONS = { agent: TERMINAL, edit: PERSON, checkpoint: FLAG };

interface Props {
  workspace: WorkspaceName;
  turns: TurnDto[];
  /** The ids of the turns the review shows, if it shows some. */
  shown?: number[];
  /** `@`'s last graph line. */
  below: Cell[];
  width: number;
  onError(message: string): void;
}

/** One row per turn and checkpoint under `@`, newest first, while they're open. */
export function TurnRows({ workspace, turns, shown, below, width, onError }: Props) {
  const open = turnsOpen.use();
  /** The row a drag or shift-click range runs from. */
  const anchor = useRef<number>(undefined);
  /** The row a drag started on, while the button is down. */
  const drag = useRef<number>(undefined);
  /** The rows a drag is across, once it reaches a second row. */
  const [dragged, setDragged] = useState<[number, number]>();
  const draggedRef = useRef(dragged);
  draggedRef.current = dragged;
  /** A drag just ended, so the click that ends it opens nothing more. */
  const ended = useRef(false);

  const openRange = (a: number, b: number) =>
    openTurns(workspace, turns.slice(Math.min(a, b), Math.max(a, b) + 1));

  useEffect(() => {
    const up = () => {
      drag.current = undefined;
      const d = draggedRef.current;
      if (!d) return;
      ended.current = true;
      setDragged(undefined);
      openRange(...d);
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  });

  if (!open) return null;

  function click(e: React.MouseEvent, i: number) {
    if (ended.current) {
      ended.current = false;
      return;
    }
    if (e.shiftKey && anchor.current !== undefined) return openRange(anchor.current, i);
    anchor.current = i;
    openTurns(workspace, [turns[i]]);
  }

  /** The rows the bracket marks: a drag's, else the review's. */
  const showing = turns.flatMap((t, i) => (shown?.includes(t.id) ? [i] : []));
  const marked = dragged ?? [showing[0] ?? -1, showing.at(-1) ?? -1];
  const [lo, hi] = [Math.min(...marked), Math.max(...marked)];

  async function undo(t: TurnDto) {
    const ok = await confirmAction(
      `Undo "${turnName(t)}"?`,
      t.kind === "agent" ? "Undo Turn" : "Undo Edit",
    );
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

  /** Moves `picked` into their own commit, between the turns before and after them. */
  async function split(picked: TurnDto[]) {
    try {
      await unwrap(
        commands.splitAtTurns(
          workspace,
          picked.map((p) => p.id),
        ),
      );
    } catch (e) {
      onError(String(e instanceof Error ? e.message : e));
    }
    reloadTurns(workspace);
  }

  function menu(e: React.MouseEvent, t: TurnDto, i: number) {
    const picked = i >= lo && i <= hi ? turns.slice(lo, hi + 1) : [t];
    const turn = t.kind === "agent";
    const undoable = t.kind !== "checkpoint";
    openMenu(e, [
      { label: "Review", onSelect: () => openTurns(workspace, [t]) },
      undoable && {
        label: turn ? "Undo Turn" : "Undo Edit",
        disabled: t.running,
        onSelect: () => undo(t),
      },
      {
        label: "Move to New Commit",
        disabled: picked.some((p) => p.running),
        onSelect: () => split(picked),
      },
      "-",
      turn && { label: "Copy Prompt", onSelect: () => copy(t.prompt ?? "") },
      turn && "-",
      { label: "Remove", onSelect: () => remove(t) },
    ]);
  }

  return turns.map((t, i) => (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the log takes the keys
    <div
      key={t.id}
      className={`lt${shown?.includes(t.id) ? " cursor" : ""}${t.running ? " running" : ""}`}
      onMouseDown={(e) => {
        e.stopPropagation();
        if (e.button === 0 && !e.shiftKey) drag.current = i;
      }}
      onMouseEnter={() => {
        if (drag.current !== undefined && (dragged || i !== drag.current))
          setDragged([drag.current, i]);
      }}
      onClick={(e) => click(e, i)}
      onContextMenu={(e) => menu(e, t, i)}
    >
      {lo >= 0 && i >= lo && i <= hi && (
        <span
          className={`lr-bk${i === lo ? " top" : ""}${i === hi ? " bot" : ""}${dragged ? " loading" : ""}`}
        />
      )}
      <GraphThrough below={below} width={width} />
      <span className="lt-body">
        <svg className={`lt-ic ${t.kind}`} viewBox="0 0 14 14" aria-hidden="true">
          <path d={ICONS[t.kind]} />
        </svg>
        <span className="lt-p">{turnName(t)}</span>
        <span className="lt-add">+{t.added}</span>
        <span className="lt-del">−{t.removed}</span>
        <span className="lt-t">{turnTime(t)}</span>
      </span>
    </div>
  ));
}
