import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeDto, View } from "../bindings";
import { editorAt } from "../editor/at";
import { openFileIn } from "../editor/open";
import { PrNote } from "../github/PrNote";
import { usePrReview } from "../github/usePrReview";
import { commands } from "../ipc";
import { agentsOff } from "../tools/agents";
import { openMenu } from "../ui/contextMenu";
import { openArchitecture } from "../workbench/panels";
import { remembered } from "../workbench/remembered";
import { useWorkspace } from "../workspaces/context";
import { CommentBlock } from "./CommentBlock";
import { Conflicts } from "./Conflicts";
import { commentSlots } from "./comments";
import { DiffLines } from "./DiffLines";
import { fileMenu } from "./fileMenu";
import { MarkMenu } from "./MarkMenu";
import { useMarkSetup } from "./marks";
import { buildLines, changedRows, type DisplayLine, type ViewMode } from "./model";
import { PrCommentsHead, prLines } from "./PrCommentsHead";
import { ReviewBar, ReviewNotes } from "./ReviewBar";
import { startRevset, workspaceRevset } from "./revset";
import { type Filter, filterLabel, plural, sumProgress } from "./state";
import { useAgentRequests } from "./useAgentRequests";
import { useCollapse } from "./useCollapse";
import { useComments } from "./useComments";
import { useContextLines } from "./useContextLines";
import { usePublishReview } from "./usePublishReview";
import { usePublishTarget } from "./usePublishTarget";
import { useReview } from "./useReview";
import { useReviewActions } from "./useReviewActions";
import { useReviewKeys } from "./useReviewKeys";
import { useReviewNav } from "./useReviewNav";
import { useReviewState } from "./useReviewState";
import { useRowMouse } from "./useRowMouse";
import { useSavedView } from "./useSavedView";
import { useSelection } from "./useSelection";
import { useSendText } from "./useSendText";
import { useWalkthrough } from "./useWalkthrough";
import { runWalkthrough, useWalkthroughRun } from "./walkthroughRun";

/** Rows changed since they were reviewed show amber, in every review. */
const amberRows = remembered("lgtm:amberRows", true, [true, false]);

import "./review.css";

const NO_CHANGES: ChangeDto[] = [];

/** Whether a line belongs to a hunk that za can collapse. */
const inHunk = (line: DisplayLine | undefined): line is Exclude<DisplayLine, { kind: "file" }> =>
  !!line && line.kind !== "file" && line.hunk >= 0;

/** Clicks on these keep their own focus instead of giving it to the review. */
function keepsFocus(t: EventTarget) {
  return (
    (t instanceof Element && !!t.closest(".comment-block, .note-block")) ||
    t instanceof HTMLInputElement ||
    t instanceof HTMLButtonElement ||
    t instanceof HTMLSelectElement
  );
}

export function ReviewPanel({ api, containerApi, params }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const [revset, setRevset] = useState(() => workspaceRevset(workspace) ?? "@");
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview(workspace);
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [scrollTo, setScrollTo] = useState<{ line: number }>();
  const keepFile = useRef<(file: number) => void>(undefined);

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const focusRoot = useCallback(() => rootRef.current?.focus({ preventScroll: true }), []);

  const review = loaded?.review;
  const setup = useMarkSetup();
  const { files, index, rowState, progress } = useReviewState(review, setup.done);
  /** Whether rows changed since they were marked or their file was checked show amber. */
  const amber = amberRows.use();
  const shown = useMemo(
    () => (amber ? index : { ...index, amber: new Set<string>() }),
    [amber, index],
  );
  const total = useMemo(() => sumProgress(progress), [progress]);
  const fold = useCollapse(files, index.checks);
  const context = useContextLines(review, setError);
  const { sendText, sendRecent, picker } = useSendText(workspace, setError, focusRoot);
  const pr = usePrReview(workspace, review, { onState: setState, onError: setError, load });
  const c = useComments({
    review,
    files,
    moves: loaded?.moves,
    onState: setState,
    onError: setError,
    send: sendText,
    onDone: focusRoot,
    canDraft: !!pr.pr,
  });
  const slots = useMemo(() => commentSlots(c.comments, c.draft), [c.comments, c.draft]);
  const w = useWalkthrough(review, files, commentOn, !!pr.pr);
  const { collapsedFiles, collapsedHunks, revealed } = fold;
  const fileLines = useMemo(
    () =>
      buildLines(files, {
        mode,
        collapsedFiles,
        collapsedHunks,
        filter,
        rowState,
        done: setup.done,
        revealed,
        comments: slots,
        notes: w.slots,
        context: context.lines,
      }),
    [
      files,
      mode,
      collapsedFiles,
      collapsedHunks,
      filter,
      rowState,
      setup.done,
      revealed,
      slots,
      w.slots,
      context.lines,
    ],
  );
  const lines = useMemo(() => [...prLines(c.onPr), ...fileLines], [c.onPr, fileLines]);
  const navComments = useMemo(() => [...c.onPr, ...c.comments], [c.onPr, c.comments]);
  const s = useSelection(files, lines, loaded?.moves);
  const { cursor, sel, place } = s;
  const changedCount = useMemo(
    () => changedRows(files, lines, sel.start, sel.end, true).length,
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
    edited: setState,
    followChecks(checked) {
      // A file checked or unchecked on its own keeps its header in place as it folds.
      if (checked.length === 1) keepFile.current?.(checked[0]);
      fold.followChecks(checked);
    },
    sendText,
    sendRecent,
    closeMenu,
    primary: setup.primary,
    openInEditor,
    collapseHunk() {
      const line = lines[cursor];
      if (inHunk(line)) fold.toggleHunk(line.file, line.hunk);
    },
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

  const commentFolds = useSavedView({
    api,
    params,
    review: review?.id,
    revset,
    files,
    lines,
    cursor,
    fold,
    scrollRef,
    place,
    setScrollTo,
  });

  // Keys go to the review as soon as its tab is picked, as they do for a terminal.
  useEffect(() => {
    if (api.isActive) focusRoot();
    const sub = api.onDidActiveChange(({ isActive }) => isActive && focusRoot());
    return () => sub.dispose();
  }, [api, focusRoot]);

  function changeFilter(next: Filter) {
    setFilter(next);
    fold.clearRevealed();
    focusRoot();
  }

  function changeView(view: View) {
    fold.clearRevealed();
    refresh(view);
    focusRoot();
  }

  const { jump, jumpToComment, jumpToRow, stepNote, stepHunk, stepFile, showLine } = useReviewNav({
    review: review?.id,
    lines,
    files,
    comments: navComments,
    notes: w.notes,
    cursor,
    place,
    setScrollTo,
    api,
    containerApi,
    focusRoot,
  });

  async function commentOn(
    file: number,
    rows: number[],
    body: string,
    snippet: string,
    draft = false,
  ) {
    if (!review) return false;
    const refs = rows.map((row) => ({ file, row }));
    const r = await commands.addComment(review.id, refs, body, snippet, draft);
    if (r.status === "error") {
      setError(r.error);
      return false;
    }
    setState(r.data);
    return true;
  }

  function runWalkthroughNow() {
    if (!review) return;
    if (agentsOff()) return setError("Walkthrough needs Claude Code, which is turned off.");
    const id = review.id;
    runWalkthrough(workspace, id, revset, (state) => setState(state, id));
  }

  function openInEditor() {
    const at = editorAt(files, lines[cursor]);
    if (at) openFileIn(containerApi, at.path, at.line);
  }

  const isCurrent = usePublishReview(
    api,
    workspace,
    review,
    { files, progress, rowState, done: setup.done, checks: index.checks, filter },
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
      apply: setState,
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

  function openFileMenu(e: React.MouseEvent, file: number) {
    const deps = {
      workspace,
      files,
      checks: index.checks,
      progress,
      collapsedFiles,
      setup,
      toggleFile: fold.toggleFile,
      setChecked: a.setChecked,
      markFile: a.markFile,
      clearFile: a.clearFile,
      openFile: (path: string) => openFileIn(containerApi, path),
    };
    openMenu(e, fileMenu(deps, file));
  }

  const onKeyDown = useReviewKeys({
    files,
    lines,
    s,
    setup,
    menu: !!menu,
    closeMenu,
    pick: a.pick,
    step: a.step,
    copy: a.copy,
    sendToTerminal: () => a.sendSelection(true),
    applyMark: a.applyMark,
    openMenuAtCursor,
    startComment: a.startComment,
    startDraft: () => a.startComment(true),
    openArchitecture: () => openArchitecture(containerApi),
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
        onDone={focusRoot}
        setup={setup}
        onMark={a.applyMark}
        onReset={a.resetMarks}
        walkthrough={walkthrough}
      />
      <ReviewNotes review={review} stale={stale} />
      <PrNote p={pr} />
      {review && <Conflicts changes={review.changes} send={(text) => sendText(text, focusRoot)} />}
      {error && <div className="review-error">{error}</div>}
      {review && fileLines.length === 0 && (
        <div className="placeholder">
          {review.files.length === 0
            ? `No changes in ${review.revset}`
            : `No ${filterLabel(filter, setup)} rows`}
        </div>
      )}
      {review && (
        <DiffLines
          files={files}
          lines={lines}
          mode={mode}
          index={shown}
          amber={amber}
          onToggleAmber={() => amberRows.set(!amber)}
          setup={setup}
          progress={progress}
          selection={sel}
          cursor={cursor}
          collapsedFiles={collapsedFiles}
          collapsedHunks={collapsedHunks}
          scrollRef={scrollRef}
          scrollTo={scrollTo}
          keepFile={keepFile}
          onBackgroundMouseDown={() => place(cursor)}
          onClearFile={a.clearFile}
          onFileMenu={openFileMenu}
          onShowContext={context.show}
          renderComment={(id) => (
            <>
              <PrCommentsHead comments={c.onPr} id={id} />
              <CommentBlock
                id={id}
                comment={navComments.find((x) => x.id === id)}
                c={c}
                folds={commentFolds}
              />
            </>
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
          setup={setup}
          canDraft={c.canDraft}
          inHunk={inHunk(lines[cursor])}
          onPick={a.pick}
          onClose={closeMenu}
        />
      )}
    </div>
  );
}
