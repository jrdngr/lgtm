import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeDto, View } from "../bindings";
import { isMac } from "../core/keys";
import { openArchitecture } from "../core/panels";
import { remembered } from "../core/remembered";
import { useWorkspace } from "../core/workspace";
import { editorAt } from "../editor/at";
import { openFileIn } from "../editor/open";
import { FindBar } from "../find/FindBar";
import { useReviewFind } from "../find/useReviewFind";
import { PrNote } from "../github/PrNote";
import { usePrReview } from "../github/usePrReview";
import { commands } from "../ipc";
import { searchItems } from "../search/menu";
import { useInfoLines } from "../symbolInfo/useInfoLines";
import { useTurns } from "../turns/turnsStore";
import { openMenu } from "../ui/contextMenu";
import { CommentBlock } from "./CommentBlock";
import { commentSlots } from "./comments";
import { useCurrentReview } from "./current";
import { DiffLines } from "./DiffLines";
import { fileMenu } from "./fileMenu";
import { MarkMenu } from "./MarkMenu";
import { useMarkSetup } from "./marks";
import { buildLines, changedRows, type DisplayLine, type ViewMode } from "./model";
import { PrCommentsHead, prLines } from "./PrCommentsHead";
import { ReviewBar, ReviewNotes } from "./ReviewBar";
import { startRevset, takeStartView, workspaceRevset } from "./revset";
import type { Filter } from "./rowState";
import { filterLabel, plural, sumProgress } from "./state";
import { useAgentRequests } from "./useAgentRequests";
import { useCollapse } from "./useCollapse";
import { useComments } from "./useComments";
import { useContextLines } from "./useContextLines";
import { useNotes } from "./useNotes";
import { usePublishReview } from "./usePublishReview";
import { usePublishTarget } from "./usePublishTarget";
import { useReview } from "./useReview";
import { useReviewActions } from "./useReviewActions";
import { useReviewKeys } from "./useReviewKeys";
import { useReviewNav } from "./useReviewNav";
import { useReviewState } from "./useReviewState";
import { type MenuAt, useRowMouse } from "./useRowMouse";
import { useSavedView } from "./useSavedView";
import { useSelection } from "./useSelection";
import { useSendText } from "./useSendText";
import { useWorkingCopy } from "./workingCopy";

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
    (t instanceof Element && !!t.closest(".comment-block")) ||
    t instanceof HTMLInputElement ||
    t instanceof HTMLButtonElement ||
    t instanceof HTMLSelectElement
  );
}

export function ReviewPanel({ api, containerApi, params }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const notes = useNotes(useCurrentReview(workspace), workspace);
  const [revset, setRevset] = useState(() => workspaceRevset(workspace) ?? "@");
  const { loaded, loading, error, setError, stale, open, refresh, setState } = useReview(workspace);
  const [mode, setMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState<Filter>("all");
  const [menu, setMenu] = useState<MenuAt>();
  const [scrollTo, setScrollTo] = useState<{ line: number; center?: boolean }>();
  const keepFile = useRef<(file: number) => void>(undefined);

  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const focusRoot = useCallback(() => rootRef.current?.focus({ preventScroll: true }), []);

  const review = loaded?.review;
  const turns = useTurns(workspace);
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
  /** Folds or unfolds a file with its header staying where it is on screen. */
  const toggleFile = (file: number) => {
    keepFile.current?.(file);
    fold.toggleFile(file);
  };
  const context = useContextLines(review, setError);
  const { sendText, sendRecent, picker } = useSendText(workspace, setError, focusRoot);
  const pr = usePrReview(workspace, review, { onState: setState, onError: setError, load });
  const c = useComments({
    review,
    files,
    moves: loaded?.moves,
    onState: setState,
    onError: setError,
    workspace,
    send: sendText,
    onDone: focusRoot,
    canDraft: !!pr.pr,
  });
  const slots = useMemo(() => commentSlots(c.comments, c.draft), [c.comments, c.draft]);
  const { collapsedFiles, collapsedHunks, revealed } = fold;
  const wc = useWorkingCopy(workspace)?.commitId;
  const atWorkingCopy = !!review && wc === review.to;
  const info = useInfoLines(workspace, files, atWorkingCopy);
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
        infos: info.open,
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
      info.open,
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

  /** Loads `target` as the review, in `view` or Combined. Returns the error message when it
   * fails. */
  async function load(target: string, view?: View): Promise<string | undefined> {
    const r = await open(target, view);
    if (typeof r === "string") return r;
    setRevset(target);
    fold.reset();
    return undefined;
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once on open
  useEffect(() => {
    const view = takeStartView(workspace);
    startRevset(workspace).then((r) => load(r, view));
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

  const { jump, jumpToComment, jumpToRow, stepHunk, stepFile, showLine } = useReviewNav({
    lines,
    files,
    comments: navComments,
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
      refetchPr: pr.refetch,
      pullPr: pr.pull,
      apply: setState,
    },
  );

  useAgentRequests(workspace, { isCurrent, show: showLine, load });
  usePublishTarget(
    api,
    workspace,
    {
      revset,
      changes: review?.changes ?? NO_CHANGES,
      loading,
      stale: !!stale,
      turn: review?.view.kind === "turn" ? review.view.turn : undefined,
    },
    { load, view: changeView, focus: focusRoot },
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
      toggleFile,
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
    stepHunk,
    stepFile,
    toggleFile,
    toggleHunk: fold.toggleHunk,
    toggleChecked: (file) => a.setChecked(file, index.checks[file] !== "checked"),
  });

  const find = useReviewFind({
    files,
    lines,
    mode,
    scrollRef,
    scrollTo: setScrollTo,
    focus: focusRoot,
  });
  function onReviewKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      find.show();
    } else onKeyDown(e);
  }

  const handlers = useRowMouse({
    files,
    lines,
    s,
    rootRef,
    setMenu,
    toggleHunk: fold.toggleHunk,
    toggleFile,
    setChecked: a.setChecked,
    reveal: fold.reveal,
  });

  return (
    <div
      ref={rootRef}
      className="review"
      tabIndex={-1}
      onKeyDown={onReviewKey}
      onMouseDown={(e) => {
        if (!keepsFocus(e.target)) focusRoot();
      }}
    >
      <ReviewBar
        review={review}
        turns={turns}
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
        send={{ count: notes.unsent.length, onSend: notes.sendOpen }}
      />
      <ReviewNotes review={review} stale={stale} />
      <PrNote p={pr} />
      {error && <div className="review-error">{error}</div>}
      {review && fileLines.length === 0 && (
        <div className="placeholder">
          {review.files.length === 0
            ? `No changes in ${review.revset}`
            : `No ${filterLabel(filter, setup)} rows`}
        </div>
      )}
      {review && find.open && (
        <div className="find-anchor">
          <FindBar {...find.bar} />
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
          renderInfo={info.render}
          renderFlag={info.flag}
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
          symbol={searchItems(workspace, menu.word ?? "", menu.path, menu.line)}
          onPick={a.pick}
          onClose={closeMenu}
        />
      )}
    </div>
  );
}
