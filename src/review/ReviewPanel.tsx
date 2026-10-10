import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MarkState, StateDto, View } from "../bindings";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import { type CurrentReview, publishReview, retractReview } from "./current";
import { DiffLines } from "./DiffLines";
import { DISABLED, MarkMenu, MENU_KEYS, type MenuAction } from "./MarkMenu";
import {
  buildLines,
  changedRows,
  flatten,
  gapKey,
  hunkKey,
  symbolMarkers,
  symbolRange,
  type ViewMode,
} from "./model";
import { ReviewBar, ReviewNotes } from "./ReviewBar";
import {
  expand,
  fileSelection,
  hunkSelection,
  rangeSelection,
  reviewSelection,
  units,
} from "./selection";
import { type Filter, fileProgress, indexState, rowStates, sumProgress } from "./state";
import { useReview } from "./useReview";
import { nextRowLine, useSelection } from "./useSelection";
import "./review.css";

const DEFAULT_REVSET = "@";

type Params = { revset?: string };

export function ReviewPanel({ api, params }: IDockviewPanelProps<Params>) {
  const [revset, setRevset] = useState(params.revset ?? DEFAULT_REVSET);
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview();
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [collapsedHunks, setCollapsedHunks] = useState<ReadonlySet<string>>(new Set());
  /** Files opened or closed by hand, by path. Others follow their check and skip status. */
  const [fileOpen, setFileOpen] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [scrollTo, setScrollTo] = useState<{ line: number }>();

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const pendingZ = useRef(false);
  const published = useRef<CurrentReview>(undefined);
  const jumpRef = useRef<(file: number) => void>(() => {});

  const review = loaded?.review;
  // biome-ignore lint/correctness/useExhaustiveDependencies: marking replaces only `state`
  const files = useMemo(() => (review ? flatten(review) : []), [review?.files]);
  const markers = useMemo(() => files.map(symbolMarkers), [files]);
  const index = useMemo(
    () => indexState(review?.state ?? { marks: [], checks: [], amber: [] }),
    [review?.state],
  );
  const rowState = useMemo(() => rowStates(files, index), [files, index]);
  const progress = useMemo(() => fileProgress(files, rowState), [files, rowState]);
  const total = useMemo(() => sumProgress(progress), [progress]);
  const collapsedFiles = useMemo(() => {
    const out = new Set<string>();
    files.forEach((f, i) => {
      const byDefault = index.checks[i] === "checked" || f.dto.skipped;
      if (!(fileOpen.get(f.dto.path) ?? !byDefault)) out.add(f.dto.path);
    });
    return out;
  }, [files, index, fileOpen]);
  const lines = useMemo(
    () => buildLines(files, { mode, collapsedFiles, collapsedHunks, filter, rowState, revealed }),
    [files, mode, collapsedFiles, collapsedHunks, filter, rowState, revealed],
  );
  const s = useSelection(files, lines, loaded?.moves);
  const { cursor, sel, select, place } = s;
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end).length,
    [files, lines, sel],
  );

  async function load(target: string) {
    const r = await open(target);
    if (!r) return;
    setCollapsedHunks(new Set());
    setFileOpen(new Map());
    setRevealed(new Set());
    api.updateParameters({ revset: target });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once on open
  useEffect(() => {
    load(revset);
  }, []);

  // A newly opened review starts on its first row.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the review is replaced
  useEffect(() => {
    place(
      Math.max(
        lines.findIndex((l) => l.kind === "rows"),
        0,
      ),
    );
  }, [review?.id]);

  useEffect(() => {
    const publish = () => {
      published.current = {
        files,
        progress,
        checks: index.checks,
        filter,
        jump: (f) => jumpRef.current(f),
      };
      publishReview(published.current);
    };
    if (review && (api.isActive || !published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    return () => sub.dispose();
  }, [api, review, files, progress, index, filter]);

  useEffect(() => () => retractReview(published.current), []);

  useEffect(() => {
    const up = () => {
      dragging.current = false;
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, []);

  function changeFilter(next: Filter) {
    setFilter(next);
    setRevealed(new Set());
  }

  function toggleHunk(file: number, hunk: number) {
    const key = hunkKey(files[file].dto.path, hunk);
    const next = new Set(collapsedHunks);
    if (!next.delete(key)) next.add(key);
    setCollapsedHunks(next);
  }

  function toggleFile(file: number) {
    const path = files[file].dto.path;
    setFileOpen(new Map(fileOpen).set(path, collapsedFiles.has(path)));
  }

  function applied(state: StateDto, files: number[] = []) {
    setState(state);
    // Checking collapses a file and unchecking expands it, whatever was done by hand.
    if (files.length) {
      const next = new Map(fileOpen);
      for (const f of files) next.delete(review?.files[f]?.path ?? "");
      setFileOpen(next);
    }
  }

  async function applyMark(state: MarkState | null) {
    if (!review) return;
    const rows = changedRows(files, lines, sel.start, sel.end);
    if (rows.length === 0) return;
    const line = lines[sel.start];
    const check = sel.level === "file" && state === "reviewed" && line ? [line.file] : [];
    const r = await commands.markRows(review.id, rows, state, check);
    if (r.status !== "ok") {
      setError(r.error);
      return;
    }
    s.setExtending(false);
    place(cursor);
    applied(r.data, check);
  }

  async function setChecked(file: number, checked: boolean) {
    if (!review) return;
    const r = await commands.setFilesChecked(review.id, [file], checked);
    if (r.status === "ok") applied(r.data, [file]);
    else setError(r.error);
  }

  jumpRef.current = (file: number) => {
    const header = lines.findIndex((l) => l.kind === "file" && l.file === file);
    if (header < 0) return;
    const next = lines[header + 1];
    const at = next?.kind === "file" ? header : nextRowLine(lines, header, 1);
    place(at);
    setScrollTo({ line: header });
    api.setActive();
    rootRef.current?.focus({ preventScroll: true });
  };

  async function step(undo: boolean) {
    if (!review) return;
    const r = await (undo ? commands.undoMarks(review.id) : commands.redoMarks(review.id));
    if (r.status === "ok") setState(r.data);
    else setError(r.error);
  }

  async function resetMarks() {
    if (!review) return;
    const r = await commands.resetReviewMarks(review.id);
    if (r.status === "ok") setState(r.data);
    else setError(r.error);
  }

  function changeView(view: View) {
    setRevealed(new Set());
    refresh(view);
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

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
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
      if (line?.kind === "file") toggleFile(line.file);
      else if (line) toggleHunk(line.file, line.hunk);
      e.preventDefault();
      return;
    }
    pendingZ.current = e.key === "z";
    switch (e.key) {
      case "j":
      case "ArrowDown":
        s.move(1, s.extending);
        break;
      case "k":
      case "ArrowUp":
        s.move(-1, s.extending);
        break;
      case "J":
        if (!s.extending && sel.level !== "range") s.setAnchor(cursor);
        s.move(1, true);
        break;
      case "K":
        if (!s.extending && sel.level !== "range") s.setAnchor(cursor);
        s.move(-1, true);
        break;
      case "v":
        s.setExtending((x) => !x);
        s.setAnchor(cursor);
        break;
      case "e": {
        const next = expand(files, lines, cursor, sel);
        if (next) {
          s.setShrinkStack([...s.shrinkStack, sel]);
          select(next, cursor, true);
        }
        break;
      }
      case "E": {
        const prev = s.shrinkStack.at(-1);
        if (prev) {
          s.setShrinkStack(s.shrinkStack.slice(0, -1));
          select(prev, cursor, true);
        }
        break;
      }
      case "Escape":
        s.setExtending(false);
        place(cursor);
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
        select(rangeSelection(s.anchor, i), i);
        return;
      }
      place(i);
      dragging.current = true;
    },
    onRowMouseEnter(i: number) {
      if (dragging.current) select(rangeSelection(s.anchor, i), i);
    },
    onRowContextMenu(e: React.MouseEvent, i: number) {
      e.preventDefault();
      if (i < sel.start || i > sel.end) place(i);
      const root = rootRef.current?.getBoundingClientRect();
      if (root) setMenu({ x: e.clientX - root.left, y: e.clientY - root.top });
    },
    onFileClick(file: number) {
      const f = fileSelection(files, lines, file);
      select(f, f.start);
    },
    onHunkClick(file: number, hunk: number) {
      const h = hunkSelection(lines, file, hunk);
      select(h, h.start);
    },
    onToggleHunk: toggleHunk,
    onToggleFile: toggleFile,
    onCheckFile: setChecked,
    onGapClick(file: number, hunk: number, firstRow: number) {
      setRevealed(new Set(revealed).add(gapKey(file, hunk, firstRow)));
    },
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
        const t = e.target;
        if (
          !(
            t instanceof HTMLInputElement ||
            t instanceof HTMLButtonElement ||
            t instanceof HTMLSelectElement
          )
        ) {
          rootRef.current?.focus({ preventScroll: true });
        }
      }}
    >
      <ReviewBar
        review={review}
        loading={loading}
        revset={revset}
        onRevset={setRevset}
        onLoad={() => load(revset)}
        onPick={(picked) => {
          setRevset(picked);
          load(picked);
        }}
        onView={changeView}
        mode={mode}
        onMode={setMode}
        filter={filter}
        onFilter={changeFilter}
        progress={total}
        onReset={resetMarks}
      />
      <ReviewNotes review={review} stale={stale} />
      {error && <div className="review-error">{error}</div>}
      {review && lines.length === 0 && (
        <div className="placeholder">
          {review.files.length === 0 ? `No changes in ${review.revset}` : `No ${filter} rows`}
        </div>
      )}
      {review && (
        <DiffLines
          files={files}
          markers={markers}
          lines={lines}
          mode={mode}
          index={index}
          progress={progress}
          selection={sel}
          cursor={cursor}
          collapsedFiles={collapsedFiles}
          collapsedHunks={collapsedHunks}
          scrollRef={scrollRef}
          scrollTo={scrollTo}
          onBackgroundMouseDown={() => place(cursor)}
          {...handlers}
        />
      )}
      <div className="review-status">
        <span>{sel.label}</span>
        <span>
          {changedCount} changed {changedCount === 1 ? "row" : "rows"}
        </span>
        {s.extending && <span className="extending">extend</span>}
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
