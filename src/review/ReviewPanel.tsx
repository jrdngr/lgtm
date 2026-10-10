import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MarkState, StateDto, View } from "../bindings";
import { commands } from "../ipc";
import { sendToTerminal, type TerminalChoice, workspaceTerminals } from "../terminal/registry";
import { isMac } from "../workbench/keys";
import { useWorkspace } from "../workspaces/context";
import { marksChanged } from "../workspaces/useWorkspaces";
import { CommentBlock, NOT_AT_WORKING_COPY } from "./CommentBlock";
import { commentSlots, draftFor } from "./comments";
import { type CurrentReview, publishReview, retractReview } from "./current";
import { DiffLines } from "./DiffLines";
import { MarkMenu, MENU_KEYS, type MenuAction } from "./MarkMenu";
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
import { selectionText } from "./sendText";
import { type Filter, fileProgress, indexState, rowStates, sumProgress } from "./state";
import { TerminalPicker } from "./TerminalPicker";
import { useComments } from "./useComments";
import { useReview } from "./useReview";
import { nextRowLine, useSelection } from "./useSelection";
import "./review.css";

const DEFAULT_REVSET = "@";

/** Drops any browser text selection so it doesn't linger next to a line selection. */
function clearTextSelection() {
  window.getSelection()?.removeAllRanges();
}

type Params = { revset?: string };

export function ReviewPanel({ api, params }: IDockviewPanelProps<Params>) {
  const workspace = useWorkspace();
  const [revset, setRevset] = useState(params.revset ?? DEFAULT_REVSET);
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview(workspace);
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [collapsedHunks, setCollapsedHunks] = useState<ReadonlySet<string>>(new Set());
  /** Files opened or closed by hand, by path. Others follow their check and skip status. */
  const [fileOpen, setFileOpen] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [revisions, setRevisions] = useState(false);
  const [scrollTo, setScrollTo] = useState<{ line: number }>();
  /** Text waiting for the user to pick which terminal it goes to. */
  const [picking, setPicking] = useState<{
    text: string;
    terminals: TerminalChoice[];
    onSent?: () => void;
  }>();

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const pendingZ = useRef(false);
  const published = useRef<CurrentReview>(undefined);
  const jumpRef = useRef<(file: number) => void>(() => {});
  const jumpToCommentRef = useRef<(id: number) => void>(() => {});
  const sendRef = useRef<(text: string) => void>(() => {});

  const review = loaded?.review;
  // biome-ignore lint/correctness/useExhaustiveDependencies: marking replaces only `state`
  const files = useMemo(() => (review ? flatten(review) : []), [review?.files]);
  const markers = useMemo(() => files.map(symbolMarkers), [files]);
  const index = useMemo(
    () => indexState(review?.state ?? { marks: [], checks: [], amber: [], comments: [] }),
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
  const c = useComments({
    review,
    files,
    moves: loaded?.moves,
    onState: edited,
    onError: setError,
    send: sendText,
    onDone: () => rootRef.current?.focus({ preventScroll: true }),
    onChanged: () => refresh(),
  });
  const slots = useMemo(() => commentSlots(c.comments, c.draft), [c.comments, c.draft]);
  const lines = useMemo(
    () =>
      buildLines(files, {
        mode,
        collapsedFiles,
        collapsedHunks,
        filter,
        rowState,
        revealed,
        comments: slots,
      }),
    [files, mode, collapsedFiles, collapsedHunks, filter, rowState, revealed, slots],
  );
  const s = useSelection(files, lines, loaded?.moves);
  const { cursor, sel, select, place } = s;
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end).length,
    [files, lines, sel],
  );

  /** Loads `target` as the review. Returns the error message when it fails. */
  async function load(target: string): Promise<string | undefined> {
    const r = await open(target);
    if (typeof r === "string") return r;
    setRevset(target);
    setCollapsedHunks(new Set());
    setFileOpen(new Map());
    setRevealed(new Set());
    api.updateParameters({ revset: target });
    return undefined;
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
        comments: review?.state.comments ?? [],
        jump: (f) => jumpRef.current(f),
        jumpToComment: (id) => jumpToCommentRef.current(id),
        send: (text) => sendRef.current(text),
      };
      publishReview(workspace, published.current);
    };
    if (review && (api.isActive || !published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    return () => sub.dispose();
  }, [api, workspace, review, files, progress, index, filter]);

  useEffect(() => () => retractReview(workspace, published.current), [workspace]);

  // Keys go to the review as soon as its tab is picked, as they do for a terminal.
  useEffect(() => {
    const focus = () => rootRef.current?.focus({ preventScroll: true });
    if (api.isActive) focus();
    const sub = api.onDidActiveChange(({ isActive }) => isActive && focus());
    return () => sub.dispose();
  }, [api]);

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

  /** Shows marks after an edit, and tells the workspace list its progress moved. */
  function edited(state: StateDto) {
    setState(state);
    marksChanged();
  }

  function applied(state: StateDto, files: number[] = []) {
    edited(state);
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
    finishAction();
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

  jumpToCommentRef.current = (id: number) => {
    const comment = c.comments.find((x) => x.id === id);
    if (!comment) return;
    const at = lines.findIndex((l) => l.kind === "comment" && l.comment === id);
    if (at < 0) {
      jumpRef.current(comment.file);
      return;
    }
    // A block under a file header has no row above it in its file.
    const line = lines[at];
    place(nextRowLine(lines, at, line.kind === "comment" && line.hunk === -1 ? 1 : -1));
    setScrollTo({ line: Math.max(at - 3, 0) });
    api.setActive();
    rootRef.current?.focus({ preventScroll: true });
  };

  async function step(undo: boolean) {
    if (!review) return;
    const r = await (undo ? commands.undoMarks(review.id) : commands.redoMarks(review.id));
    if (r.status === "ok") edited(r.data);
    else setError(r.error);
  }

  async function resetMarks() {
    if (!review) return;
    const r = await commands.resetReviewMarks(review.id);
    if (r.status === "ok") edited(r.data);
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

  /** Pastes text into the prompt of one of the workspace's terminals, asking which one when
   * there are several. Calls `onSent` once it's sent. */
  function sendText(text: string, onSent?: () => void) {
    const terminals = workspaceTerminals(workspace);
    if (terminals.length > 1) setPicking({ text, terminals, onSent });
    else sendTo(text, undefined, onSent);
  }

  function sendTo(text: string, id: string | undefined, onSent?: () => void) {
    setPicking(undefined);
    const where = workspace ? `workspace ${workspace}` : "the main workspace";
    const result = sendToTerminal(workspace, text, id);
    if (result === "noTerminal") {
      setError(`No terminal is open in ${where}. Open one with ⌘T and start your agent.`);
      return;
    }
    if (result === "unsafe") {
      setError(
        "That terminal can't take a paste safely: its program would run each line. Start your agent there first.",
      );
      return;
    }
    setError(undefined);
    onSent?.();
  }

  sendRef.current = sendText;

  /** Clears the selection to the cursor, as every action on a selection does. */
  function finishAction() {
    s.setExtending(false);
    place(cursor);
    clearTextSelection();
  }

  function sendSelection() {
    const text = selectionText(files, lines, sel.start, sel.end);
    if (text) sendText(text, finishAction);
  }

  /** Opens a comment draft on the selection, in its ask box when it's for a change. */
  function startComment(field: "body" | "ask" = "body") {
    const draft = draftFor(files, lines, sel.start, sel.end);
    if (!draft) {
      setError("Select changed rows to comment on.");
      return;
    }
    setError(undefined);
    c.start(draft, field);
    finishAction();
  }

  function pick(action: MenuAction) {
    setMenu(undefined);
    if (action === "copy") copy();
    else if (action === "agent") sendSelection();
    else if (action === "comment") startComment();
    else if (action === "change") {
      if (review?.hasWorkingCopy) startComment("ask");
      else setError(NOT_AT_WORKING_COPY);
    } else applyMark(action === "clear" ? null : (action as MarkState));
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
    const t = e.target;
    if (
      t instanceof HTMLInputElement ||
      t instanceof HTMLSelectElement ||
      t instanceof HTMLTextAreaElement
    )
      return;
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
      else if (key === "c") {
        // Selected code text copies natively; otherwise copy the selected rows.
        if (window.getSelection()?.isCollapsed === false) return;
        copy();
      } else return;
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
      case "c":
        if (!e.repeat) startComment();
        break;
      case "r":
        setRevisions(true);
        break;
      case "x": {
        const file = lines[cursor]?.file;
        if (file !== undefined && !e.repeat) setChecked(file, index.checks[file] !== "checked");
        break;
      }
      default:
        return;
    }
    e.preventDefault();
  }

  const handlers = {
    onRowMouseDown(e: React.MouseEvent, i: number) {
      if (e.button !== 0) return;
      // Only code text is selectable. Gutters and double/triple clicks pick rows, not text.
      const inCode = e.target instanceof Element && e.target.closest(".code");
      if (!inCode || e.detail >= 2) {
        e.preventDefault();
        clearTextSelection();
      }
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
        const inBlock = t instanceof Element && t.closest(".comment-block");
        if (
          !(
            inBlock ||
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
        workspace={workspace}
        review={review}
        loading={loading}
        revset={revset}
        onLoad={load}
        panel={revisions}
        onPanel={(open) => {
          setRevisions(open);
          if (!open) rootRef.current?.focus({ preventScroll: true });
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
          renderComment={(id) => (
            <CommentBlock id={id} comment={c.comments.find((x) => x.id === id)} c={c} />
          )}
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
      {picking && (
        <TerminalPicker
          terminals={picking.terminals}
          onPick={(id) => sendTo(picking.text, id, picking.onSent)}
          onClose={() => {
            setPicking(undefined);
            rootRef.current?.focus({ preventScroll: true });
          }}
        />
      )}
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
