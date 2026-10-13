import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeDto, StateDto, View } from "../bindings";
import { editorAt } from "../editor/at";
import { openFileIn } from "../editor/open";
import { PrNote } from "../github/PrNote";
import { usePrReview } from "../github/usePrReview";
import { commands } from "../ipc";
import { agentsOff } from "../tools/agents";
import { openGraph } from "../workbench/panels";
import { useWorkspace } from "../workspaces/context";
import { marksChanged } from "../workspaces/useWorkspaces";
import { CommentBlock } from "./CommentBlock";
import { Conflicts } from "./Conflicts";
import { commentNear, commentSlots } from "./comments";
import { DiffLines } from "./DiffLines";
import { MarkMenu } from "./MarkMenu";
import { buildLines, changedRows, symbolMarkers, type ViewMode } from "./model";
import { ReviewBar, ReviewNotes } from "./ReviewBar";
import { startRevset, workspaceRevset } from "./revset";
import { type Filter, plural, sumProgress } from "./state";
import { openRevisions } from "./target";
import { useAgentRequests } from "./useAgentRequests";
import { useCollapse } from "./useCollapse";
import { useComments } from "./useComments";
import { usePublishReview } from "./usePublishReview";
import { usePublishTarget } from "./usePublishTarget";
import { useReview } from "./useReview";
import { useReviewActions } from "./useReviewActions";
import { useReviewKeys } from "./useReviewKeys";
import { useReviewNav } from "./useReviewNav";
import { useReviewState } from "./useReviewState";
import { useRowMouse } from "./useRowMouse";
import { useSelection } from "./useSelection";
import { useSendText } from "./useSendText";
import { useWalkthrough } from "./useWalkthrough";
import { runWalkthrough, useWalkthroughRun } from "./walkthroughRun";
import "./review.css";

const NO_CHANGES: ChangeDto[] = [];

/** Clicks on these keep their own focus instead of giving it to the review. */
function keepsFocus(t: EventTarget) {
  return (
    (t instanceof Element && !!t.closest(".comment-block, .note-block")) ||
    t instanceof HTMLInputElement ||
    t instanceof HTMLButtonElement ||
    t instanceof HTMLSelectElement
  );
}

export function ReviewPanel({ api, containerApi }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const [revset, setRevset] = useState(() => workspaceRevset(workspace) ?? "@");
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview(workspace);
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [scrollTo, setScrollTo] = useState<{ line: number }>();

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const focusRoot = useCallback(() => rootRef.current?.focus({ preventScroll: true }), []);

  const review = loaded?.review;
  const { files, index, rowState, progress } = useReviewState(review);
  const markers = useMemo(() => files.map(symbolMarkers), [files]);
  const total = useMemo(() => sumProgress(progress), [progress]);
  const fold = useCollapse(files, index.checks);
  const { sendText, picker } = useSendText(workspace, setError, focusRoot);
  const pr = usePrReview(workspace, review, { onState: edited, onError: setError, load });
  const c = useComments({
    review,
    files,
    moves: loaded?.moves,
    onState: edited,
    onError: setError,
    send: sendText,
    onDone: focusRoot,
    onChanged: () => refresh(),
    canDraft: !!pr.pr,
  });
  const slots = useMemo(() => commentSlots(c.comments, c.draft), [c.comments, c.draft]);
  const w = useWalkthrough(review, files, commentOn);
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
        notes: w.slots,
      }),
    [files, mode, collapsedFiles, collapsedHunks, filter, rowState, revealed, slots, w.slots],
  );
  const s = useSelection(files, lines, loaded?.moves);
  const { cursor, sel, place } = s;
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end).length,
    [files, lines, sel],
  );
  const closeMenu = () => setMenu(undefined);
  const walkthrough = useWalkthroughRun(review?.id);
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
    startRevset(workspace).then(load);
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

  const { jump, jumpToComment, jumpToRow, stepNote, stepHunk, stepFile, showLine } = useReviewNav({
    review: review?.id,
    lines,
    files,
    comments: c.comments,
    notes: w.notes,
    cursor,
    place,
    setScrollTo,
    api,
    containerApi,
    focusRoot,
  });

  async function commentOn(file: number, rows: number[], body: string, snippet: string) {
    if (!review) return false;
    const refs = rows.map((row) => ({ file, row }));
    const r = await commands.addComment(review.id, refs, body, snippet, false);
    if (r.status === "error") {
      setError(r.error);
      return false;
    }
    edited(r.data);
    return true;
  }

  function runWalkthroughNow() {
    if (!review) return;
    if (agentsOff()) return setError("Walkthrough needs Claude Code, which is turned off.");
    runWalkthrough(review.id, edited);
  }

  function openInEditor() {
    const at = editorAt(files, lines[cursor]);
    if (at) openFileIn(containerApi, at.path, at.line);
  }

  const isCurrent = usePublishReview(
    api,
    workspace,
    review,
    { files, progress, rowState, checks: index.checks, filter },
    {
      jump,
      jumpToComment,
      jumpToRow,
      show: showLine,
      commentOn,
      send: sendText,
      runWalkthrough: runWalkthroughNow,
      refetchPr: pr.refetch,
      pullPr: pr.pull,
      apply: edited,
    },
  );

  useAgentRequests(workspace, { isCurrent, show: showLine, load });
  usePublishTarget(
    api,
    workspace,
    { revset, changes: review?.changes ?? NO_CHANGES, loading, stale: !!stale },
    { load, focus: focusRoot },
  );

  function openMenuAtCursor() {
    const root = rootRef.current?.getBoundingClientRect();
    const row = scrollRef.current?.querySelector(".line.cursor")?.getBoundingClientRect();
    if (!root) return;
    setMenu({
      // A row scrolled sideways starts left of the pane.
      x: Math.max(row?.left ?? root.left, root.left) - root.left + 80,
      y: (row?.bottom ?? root.top + 60) - root.top,
    });
  }

  function askAgent() {
    const id = commentNear(lines, cursor);
    if (id !== undefined && !agentsOff()) c.openAsk(id);
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
    startDraft: () => a.startComment("body", true),
    askAgent,
    openRevisions: () => openRevisions(workspace),
    openGraph: () => openGraph(containerApi),
    openInEditor,
    runWalkthrough: runWalkthroughNow,
    stepNote,
    stepHunk,
    stepFile,
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
        walkthrough={walkthrough}
      />
      <ReviewNotes review={review} stale={stale} />
      <PrNote p={pr} />
      {review && (
        <Conflicts
          workspace={workspace}
          changes={review.changes}
          send={(text) => sendText(text, focusRoot)}
          onDone={() => refresh()}
        />
      )}
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
          renderNote={w.renderNote}
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
