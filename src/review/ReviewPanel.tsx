import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MarkDto, MarkState, ReviewDto } from "../bindings";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import { type CurrentReview, publishReview, retractReview } from "./current";
import { DiffLines } from "./DiffLines";
import { DISABLED, MarkMenu, MENU_KEYS, type MenuAction } from "./MarkMenu";
import {
  buildLines,
  changedRows,
  type DisplayLine,
  flatten,
  hunkKey,
  markMap,
  symbolMarkers,
  symbolRange,
  type ViewMode,
} from "./model";
import {
  expand,
  fileSelection,
  hunkSelection,
  lineSelection,
  rangeSelection,
  reviewSelection,
  type Selection,
  units,
} from "./selection";
import "./review.css";

const DEFAULT_REVSET = "trunk()..@";

function nextRowLine(lines: DisplayLine[], from: number, dir: 1 | -1): number {
  for (let i = from + dir; i >= 0 && i < lines.length; i += dir) {
    if (lines[i].kind === "rows") return i;
  }
  return from;
}

/** Where line `i` of `from` landed in `to`, after a view mode or collapse change. */
function relocate(from: DisplayLine[], to: DisplayLine[], i: number): number {
  const line = from[i];
  if (!line) return 0;
  const found = to.findIndex((l) => {
    if (l.file !== line.file) return false;
    if (line.kind === "rows") return l.kind === "rows" && l.rows.some((r) => line.rows.includes(r));
    if (line.kind === "hunk") return l.kind === "hunk" && l.hunk === line.hunk;
    return l.kind === "file";
  });
  if (found >= 0) return found;
  // The row went into a collapsed hunk: land on its header.
  return line.kind === "rows"
    ? to.findIndex((l) => l.kind === "hunk" && l.file === line.file && l.hunk === line.hunk)
    : 0;
}

type Params = { revset?: string };

export function ReviewPanel({ api, params }: IDockviewPanelProps<Params>) {
  const [revset, setRevset] = useState(params.revset ?? DEFAULT_REVSET);
  const [review, setReview] = useState<ReviewDto>();
  const [marks, setMarks] = useState<MarkDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [mode, setMode] = useState<ViewMode>("unified");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [cursor, setCursor] = useState(0);
  const [sel, setSel] = useState<Selection>(lineSelection(0));
  const [shrinkStack, setShrinkStack] = useState<Selection[]>([]);
  const [extending, setExtending] = useState(false);
  const [anchor, setAnchor] = useState(0);
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [scrollTo, setScrollTo] = useState<{ line: number }>();

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const pendingZ = useRef(false);
  const reviewId = useRef<number>(undefined);
  const published = useRef<CurrentReview>(undefined);
  const jumpRef = useRef<(file: number) => void>(() => {});

  const files = useMemo(() => (review ? flatten(review) : []), [review]);
  const markers = useMemo(() => files.map(symbolMarkers), [files]);
  const lines = useMemo(() => buildLines(files, mode, collapsed), [files, mode, collapsed]);
  const markIndex = useMemo(() => markMap(marks), [marks]);
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end).length,
    [files, lines, sel],
  );

  function select(next: Selection, at: number = cursor, keepStack = false) {
    setSel(next);
    setCursor(at);
    if (!keepStack) setShrinkStack([]);
  }

  async function load(target: string) {
    setLoading(true);
    setError(undefined);
    const r = await commands.openReview(target);
    setLoading(false);
    if (r.status === "error") {
      setError(r.error);
      return;
    }
    if (reviewId.current !== undefined) commands.closeReview(reviewId.current);
    reviewId.current = r.data.id;
    setReview(r.data);
    setMarks(r.data.marks);
    setCollapsed(new Set());
    const first = buildLines(flatten(r.data), mode, new Set()).findIndex((l) => l.kind === "rows");
    setAnchor(Math.max(first, 0));
    select(lineSelection(Math.max(first, 0)), Math.max(first, 0));
    api.updateParameters({ revset: target });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once on open
  useEffect(() => {
    load(revset);
    return () => {
      if (reviewId.current !== undefined) commands.closeReview(reviewId.current);
    };
  }, []);

  useEffect(() => {
    const publish = () => {
      published.current = { files, marks, jump: (f) => jumpRef.current(f) };
      publishReview(published.current);
    };
    if (review && (api.isActive || !published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    return () => sub.dispose();
  }, [api, review, files, marks]);

  useEffect(() => () => retractReview(published.current), []);

  useEffect(() => {
    const up = () => {
      dragging.current = false;
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, []);

  function relayout(nextMode: ViewMode, nextCollapsed: ReadonlySet<string>) {
    const next = buildLines(files, nextMode, nextCollapsed);
    const at = relocate(lines, next, cursor);
    setMode(nextMode);
    setCollapsed(nextCollapsed);
    setAnchor(at);
    select(lineSelection(at), at);
  }

  function toggleHunk(file: number, hunk: number) {
    const key = hunkKey(file, hunk);
    const next = new Set(collapsed);
    if (!next.delete(key)) next.add(key);
    relayout(mode, next);
  }

  async function applyMark(state: MarkState | null) {
    if (!review) return;
    const rows = changedRows(files, lines, sel.start, sel.end);
    if (rows.length === 0) return;
    const r = await commands.markRows(review.id, rows, state);
    if (r.status !== "ok") {
      setError(r.error);
      return;
    }
    setMarks(r.data);
    setExtending(false);
    setAnchor(cursor);
    select(lineSelection(cursor));
  }

  jumpRef.current = (file: number) => {
    const header = lines.findIndex((l) => l.kind === "file" && l.file === file);
    if (header < 0) return;
    const next = lines[header + 1];
    const at = next?.kind === "file" ? header : nextRowLine(lines, header, 1);
    setAnchor(at);
    select(lineSelection(at), at);
    setScrollTo({ line: header });
    api.setActive();
    rootRef.current?.focus({ preventScroll: true });
  };

  async function step(undo: boolean) {
    if (!review) return;
    const r = await (undo ? commands.undoMarks(review.id) : commands.redoMarks(review.id));
    if (r.status === "ok") setMarks(r.data);
    else setError(r.error);
  }

  async function resetMarks() {
    if (!review) return;
    const r = await commands.resetReviewMarks(review.id);
    if (r.status === "ok") setMarks(r.data);
    else setError(r.error);
  }

  function copy() {
    const text: string[] = [];
    for (let i = sel.start; i <= sel.end; i++) {
      const line = lines[i];
      if (line?.kind === "rows")
        for (const r of line.rows) text.push(files[line.file].rows[r].text);
    }
    if (text.length) navigator.clipboard.writeText(text.join("\n"));
  }

  function pick(action: MenuAction) {
    setMenu(undefined);
    if (DISABLED.has(action)) return;
    if (action === "copy") copy();
    else applyMark(action === "clear" ? null : (action as MarkState));
  }

  function openMenuAtCursor() {
    const root = rootRef.current?.getBoundingClientRect();
    const row = scrollRef.current?.querySelector(".line.cursor")?.getBoundingClientRect();
    if (!root) return;
    setMenu({
      x: (row?.left ?? root.left) - root.left + 80,
      y: (row?.bottom ?? root.top + 60) - root.top,
    });
  }

  function move(dir: 1 | -1, extend: boolean) {
    const next = nextRowLine(lines, cursor, dir);
    if (extend) {
      select(rangeSelection(anchor, next), next);
    } else {
      setAnchor(next);
      select(lineSelection(next), next);
    }
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return;
    if (menu) {
      e.preventDefault();
      if (e.key === "Escape") setMenu(undefined);
      else if (MENU_KEYS[e.key] && !e.repeat) pick(MENU_KEYS[e.key]);
      return;
    }
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod) {
      const key = e.key.toLowerCase();
      if (key === "a") select(reviewSelection(lines));
      else if (key === "z") step(!e.shiftKey);
      else if (key === "c") copy();
      else return;
      e.preventDefault();
      return;
    }
    if (pendingZ.current && e.key === "a") {
      pendingZ.current = false;
      const line = lines[cursor];
      if (line && line.kind !== "file") toggleHunk(line.file, line.hunk);
      e.preventDefault();
      return;
    }
    pendingZ.current = e.key === "z";
    switch (e.key) {
      case "j":
      case "ArrowDown":
        move(1, extending);
        break;
      case "k":
      case "ArrowUp":
        move(-1, extending);
        break;
      case "J":
        if (!extending && sel.level !== "range") setAnchor(cursor);
        move(1, true);
        break;
      case "K":
        if (!extending && sel.level !== "range") setAnchor(cursor);
        move(-1, true);
        break;
      case "v":
        setExtending((x) => !x);
        setAnchor(cursor);
        break;
      case "e": {
        const next = expand(files, lines, cursor, sel);
        if (next) {
          setShrinkStack([...shrinkStack, sel]);
          select(next, cursor, true);
        }
        break;
      }
      case "E": {
        const prev = shrinkStack.at(-1);
        if (prev) {
          setShrinkStack(shrinkStack.slice(0, -1));
          select(prev, cursor, true);
        }
        break;
      }
      case "Escape":
        setExtending(false);
        setAnchor(cursor);
        select(lineSelection(cursor));
        break;
      case " ":
        if (!e.repeat) applyMark("reviewed");
        break;
      case "m":
        openMenuAtCursor();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  const handlers = {
    onRowMouseDown(e: React.MouseEvent, i: number) {
      if (e.button !== 0) return;
      if (e.detail === 2) {
        const fn = units(files, lines, i).find((u) => u.level === "function");
        if (fn) select(fn, i);
        return;
      }
      if (e.detail >= 3) {
        const line = lines[i];
        if (line.kind !== "file") select(hunkSelection(lines, line.file, line.hunk), i);
        return;
      }
      if (e.shiftKey) {
        select(rangeSelection(anchor, i), i);
        return;
      }
      setAnchor(i);
      select(lineSelection(i), i);
      dragging.current = true;
    },
    onRowMouseEnter(i: number) {
      if (dragging.current) select(rangeSelection(anchor, i), i);
    },
    onRowContextMenu(e: React.MouseEvent, i: number) {
      e.preventDefault();
      if (i < sel.start || i > sel.end) {
        setAnchor(i);
        select(lineSelection(i), i);
      }
      const root = rootRef.current?.getBoundingClientRect();
      if (root) setMenu({ x: e.clientX - root.left, y: e.clientY - root.top });
    },
    onFileClick(file: number) {
      const s = fileSelection(files, lines, file);
      select(s, s.start);
    },
    onHunkClick(file: number, hunk: number) {
      const s = hunkSelection(lines, file, hunk);
      select(s, s.start);
    },
    onToggleHunk: toggleHunk,
    onSymbolClick(i: number, file: number, row: number) {
      const marker = markers[file].get(row);
      if (!marker) return;
      const range = symbolRange(files, lines, file, marker.symbol, marker.side);
      if (range) {
        const label = `${marker.symbol.kind}: ${marker.symbol.name}`;
        select({ start: range[0], end: range[1], level: "function", label }, i);
      }
    },
  };

  return (
    <div
      ref={rootRef}
      className="review"
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onMouseDown={(e) => {
        if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLButtonElement)) {
          rootRef.current?.focus({ preventScroll: true });
        }
      }}
    >
      <div className="review-bar">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            load(revset);
          }}
        >
          <input
            className="revset"
            value={revset}
            onChange={(e) => setRevset(e.target.value)}
            spellCheck={false}
            aria-label="Revset"
          />
          <button type="submit" disabled={loading}>
            {loading ? "Loading" : "Load"}
          </button>
        </form>
        <button
          type="button"
          onClick={() => relayout(mode === "unified" ? "split" : "unified", collapsed)}
        >
          {mode === "unified" ? "Split" : "Unified"}
        </button>
        <span className="spacer" />
        <button type="button" disabled={!review} onClick={resetMarks}>
          Reset marks for this review
        </button>
      </div>
      {error && <div className="review-error">{error}</div>}
      {review && review.files.length === 0 && (
        <div className="placeholder">No changes in {review.revset}</div>
      )}
      {review && (
        <DiffLines
          files={files}
          markers={markers}
          lines={lines}
          mode={mode}
          marks={markIndex}
          selection={sel}
          cursor={cursor}
          collapsed={collapsed}
          scrollRef={scrollRef}
          scrollTo={scrollTo}
          onBackgroundMouseDown={() => select(lineSelection(cursor))}
          {...handlers}
        />
      )}
      <div className="review-status">
        <span>{sel.label}</span>
        <span>
          {changedCount} changed {changedCount === 1 ? "row" : "rows"}
        </span>
        {extending && <span className="extending">extend</span>}
      </div>
      {menu && (
        <MarkMenu
          x={menu.x}
          y={menu.y}
          changed={changedCount}
          onPick={pick}
          onClose={() => setMenu(undefined)}
        />
      )}
    </div>
  );
}
