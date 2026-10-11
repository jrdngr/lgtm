import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeDto, StateDto, View } from "../bindings";
import { commands } from "../ipc";
import { agentsOff } from "../tools/agents";
import { openGraph } from "../workbench/panels";
import { useWorkspace } from "../workspaces/context";
import { marksChanged } from "../workspaces/useWorkspaces";
import { CommentBlock } from "./CommentBlock";
import { commentSlots } from "./comments";
import { DiffLines } from "./DiffLines";
import { stepHotspot } from "./hotspots";
import { MarkMenu } from "./MarkMenu";
import {
  buildLines,
  changedRows,
  commentJumpTarget,
  fileJumpTarget,
  flatten,
  type JumpTarget,
  symbolMarkers,
  type ViewMode,
} from "./model";
import { ReviewBar, ReviewNotes } from "./ReviewBar";
import { findRisks, useRiskRun } from "./riskRun";
import { type Filter, fileProgress, indexState, plural, rowStates, sumProgress } from "./state";
import { openRevisions } from "./target";
import { useAgentRequests } from "./useAgentRequests";
import { useCollapse } from "./useCollapse";
import { useComments } from "./useComments";
import { usePublishReview } from "./usePublishReview";
import { usePublishTarget } from "./usePublishTarget";
import { useReview } from "./useReview";
import { useReviewActions } from "./useReviewActions";
import { useReviewKeys } from "./useReviewKeys";
import { useRowMouse } from "./useRowMouse";
import { useSelection } from "./useSelection";
import { useSendText } from "./useSendText";
import "./review.css";

/** A review pane always opens on the working copy, so a revset saved last time that no longer
 * resolves can't break startup. */
const START_REVSET = "@";
const NO_CHANGES: ChangeDto[] = [];
const NO_STATE: StateDto = { marks: [], checks: [], amber: [], comments: [], hotspots: [] };

/** Clicks on these keep their own focus instead of giving it to the review. */
function keepsFocus(t: EventTarget) {
  return (
    (t instanceof Element && !!t.closest(".comment-block")) ||
    t instanceof HTMLInputElement ||
    t instanceof HTMLButtonElement ||
    t instanceof HTMLSelectElement
  );
}

export function ReviewPanel({ api, containerApi }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const [revset, setRevset] = useState(START_REVSET);
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview(workspace);
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [scrollTo, setScrollTo] = useState<{ line: number }>();

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const focusRoot = useCallback(() => rootRef.current?.focus({ preventScroll: true }), []);

  const review = loaded?.review;
  // biome-ignore lint/correctness/useExhaustiveDependencies: marking replaces only `state`
  const files = useMemo(() => (review ? flatten(review) : []), [review?.files]);
  const markers = useMemo(() => files.map(symbolMarkers), [files]);
  const index = useMemo(() => indexState(review?.state ?? NO_STATE), [review?.state]);
  const rowState = useMemo(() => rowStates(files, index), [files, index]);
  const progress = useMemo(() => fileProgress(files, rowState), [files, rowState]);
  const total = useMemo(() => sumProgress(progress), [progress]);
  const fold = useCollapse(files, index.checks);
  const { sendText, picker } = useSendText(workspace, setError, focusRoot);
  const c = useComments({
    review,
    files,
    moves: loaded?.moves,
    onState: edited,
    onError: setError,
    send: sendText,
    onDone: focusRoot,
    onChanged: () => refresh(),
  });
  const slots = useMemo(() => commentSlots(c.comments, c.draft), [c.comments, c.draft]);
  const { collapsedFiles, collapsedHunks, revealed } = fold;
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
  const { cursor, sel, place } = s;
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end).length,
    [files, lines, sel],
  );
  const closeMenu = () => setMenu(undefined);
  const risk = useRiskRun(review?.id);
  const a = useReviewActions({
    review,
    files,
    lines,
    s,
    c,
    setError,
    edited,
    followChecks: fold.followChecks,
    sendText,
    closeMenu,
  });

  /** Loads `target` as the review. Returns the error message when it fails. */
  async function load(target: string): Promise<string | undefined> {
    const r = await open(target);
    if (typeof r === "string") return r;
    setRevset(target);
    fold.reset();
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

  // Keys go to the review as soon as its tab is picked, as they do for a terminal.
  useEffect(() => {
    if (api.isActive) focusRoot();
    const sub = api.onDidActiveChange(({ isActive }) => isActive && focusRoot());
    return () => sub.dispose();
  }, [api, focusRoot]);

  /** Shows marks after an edit, and tells the workspace list its progress moved. */
  function edited(state: StateDto) {
    setState(state);
    marksChanged();
  }

  function changeFilter(next: Filter) {
    setFilter(next);
    fold.clearRevealed();
  }

  function changeView(view: View) {
    fold.clearRevealed();
    refresh(view);
  }

  function jumpTo(target: JumpTarget) {
    place(target.cursor);
    setScrollTo({ line: target.top });
    api.setActive();
    focusRoot();
  }

  function jump(file: number) {
    const target = fileJumpTarget(lines, file);
    if (target) jumpTo(target);
  }

  function jumpToComment(id: number) {
    const comment = c.comments.find((x) => x.id === id);
    if (!comment) return;
    const target = commentJumpTarget(lines, id);
    if (target) jumpTo(target);
    else jump(comment.file);
  }

  function jumpToRow(file: number, row: number) {
    const at = lines.findIndex((l) => l.kind === "rows" && l.file === file && l.rows.includes(row));
    if (at < 0) return jump(file);
    jumpTo({ cursor: at, top: Math.max(at - 3, 0) });
  }

  async function commentOn(file: number, rows: number[], body: string, snippet: string) {
    if (!review) return false;
    const refs = rows.map((row) => ({ file, row }));
    const r = await commands.addComment(review.id, refs, body, snippet);
    if (r.status === "error") {
      setError(r.error);
      return false;
    }
    edited(r.data);
    return true;
  }

  function findRisksNow() {
    if (!review) return;
    if (agentsOff()) return setError("Highlight risks needs Claude Code, which is turned off.");
    findRisks(review.id, edited);
  }

  function stepHotspotFrom(dir: 1 | -1) {
    const at = stepHotspot(lines, review?.state.hotspots ?? [], cursor, dir);
    if (at !== undefined) jumpTo({ cursor: at, top: Math.max(at - 3, 0) });
  }

  /** Shows `line` of `path`: the row on that new-side line, or the nearest one. */
  function showLine(path: string, line: number | null) {
    const file = files.findIndex((f) => f.dto.path === path || f.dto.oldPath === path);
    if (file < 0) return setError(`An agent asked to show ${path}, which isn't in this review`);
    if (line === null) return jump(file);
    const rows = files[file].rows;
    let best = -1;
    rows.forEach((r, i) => {
      if (r.newLine === null) return;
      if (best < 0 || Math.abs(r.newLine - line) < Math.abs((rows[best].newLine ?? 0) - line))
        best = i;
    });
    if (best < 0) jump(file);
    else jumpToRow(file, best);
  }

  const isCurrent = usePublishReview(
    api,
    workspace,
    review,
    { files, progress, rowState, checks: index.checks, filter },
    { jump, jumpToComment, jumpToRow, commentOn, send: sendText, findRisks: findRisksNow },
  );

  useAgentRequests(workspace, { isCurrent, show: showLine, load });
  usePublishTarget(
    api,
    workspace,
    { revset, changes: review?.changes ?? NO_CHANGES, loading },
    { load, focus: focusRoot },
  );

  function openMenuAtCursor() {
    const root = rootRef.current?.getBoundingClientRect();
    const row = scrollRef.current?.querySelector(".line.cursor")?.getBoundingClientRect();
    if (!root) return;
    setMenu({
      x: (row?.left ?? root.left) - root.left + 80,
      y: (row?.bottom ?? root.top + 60) - root.top,
    });
  }

  const onKeyDown = useReviewKeys({
    files,
    lines,
    s,
    menu: !!menu,
    closeMenu,
    pick: a.pick,
    step: a.step,
    copy: a.copy,
    applyMark: a.applyMark,
    openMenuAtCursor,
    startComment: a.startComment,
    openRevisions: () => openRevisions(workspace),
    openGraph: () => openGraph(containerApi),
    findRisks: findRisksNow,
    stepHotspot: stepHotspotFrom,
    toggleFile: fold.toggleFile,
    toggleHunk: fold.toggleHunk,
    toggleChecked: (file) => a.setChecked(file, index.checks[file] !== "checked"),
  });

  const handlers = useRowMouse({
    files,
    lines,
    markers,
    s,
    rootRef,
    setMenu,
    toggleHunk: fold.toggleHunk,
    toggleFile: fold.toggleFile,
    setChecked: a.setChecked,
    reveal: fold.reveal,
  });

  return (
    <div
      ref={rootRef}
      className="review"
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onMouseDown={(e) => {
        if (!keepsFocus(e.target)) focusRoot();
      }}
    >
      <ReviewBar
        review={review}
        onView={changeView}
        mode={mode}
        onMode={setMode}
        filter={filter}
        onFilter={changeFilter}
        progress={total}
        onReset={a.resetMarks}
        onGraph={() => openGraph(containerApi)}
        risk={risk}
        onRisks={findRisksNow}
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
          {changedCount} changed {plural(changedCount)}
        </span>
        {s.extending && <span className="extending">extend</span>}
      </div>
      {picker}
      {menu && (
        <MarkMenu
          x={menu.x}
          y={menu.y}
          changed={changedCount}
          onPick={a.pick}
          onClose={closeMenu}
        />
      )}
    </div>
  );
}
