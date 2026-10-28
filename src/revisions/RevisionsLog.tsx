import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { LogRow } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { setPr, usePr } from "../github/pr";
import { commands, errorText, unwrap } from "../ipc";
import { editAction, revRowMenu } from "../review/RevisionsPanelMenu";
import { workspaceRevset } from "../review/revset";
import { useReviewTarget } from "../review/target";
import { TurnRows, TurnsToggle } from "../turns/TurnRows";
import { reloadTurns, turnsOpen, useTurns } from "../turns/turnsStore";
import { openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { pinStackPr, reviewStackPr } from "../workspaces/stack";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { BookmarkPopup } from "./BookmarkPopup";
import { CommitLines } from "./CommitLines";
import { graphOf } from "./graphCells";
import { LogGraph } from "./LogGraph";
import { loadReview, onFocusLog, onOpenMotions } from "./loadReview";
import { onLogRevsetChanged, publishLogRevset } from "./logRevset";
import { MotionPopup } from "./MotionPopup";
import { type Motion, type MotionTarget, motions, narrow, retitle, step, title } from "./motions";
import { NamePopup } from "./NamePopup";
import { PrPill } from "./PrPill";
import { RevisionPopover, useRowHover } from "./RevisionPopover";
import { RevisionsFooter } from "./RevisionsFooter";
import { RevPickPopup } from "./RevPickPopup";
import { RunningRow } from "./RunningRow";
import { useBookmarkPick } from "./useBookmarkPick";
import { useJjActions } from "./useJjActions";
import { useRevPick } from "./useRevPick";
import { type RowPr, useRowPrs } from "./useRowPrs";
import "./revisions.css";

/** Cursor moves. Shifted keys extend the range from the anchor. */
const MOVES: Record<string, number> = { j: 1, ArrowDown: 1, J: 1, k: -1, ArrowUp: -1, K: -1 };

const UNRELATED = "Not one line of ancestry";

interface Props {
  workspace: WorkspaceName;
  /** Changes when the user asks for a refresh, which asks GitHub for PRs again. */
  refreshed: number;
  /** Trunk's bookmark, for the rebase motion's label. */
  trunk: string | null;
}

/** `jj log` to pick what to review and which commit to edit. Reviewed commits are tinted with
 * a bracket, `@` green over them, empty commits yellow over both, and each open PR's head has
 * its pill. Keys act on the highlighted commit; dragging across commits reviews them, and
 * double-clicking one edits it. */
export function RevisionsLog({ workspace, refreshed, trunk }: Props) {
  const target = useReviewTarget(workspace);
  const turns = useTurns(workspace);
  const revset = target?.revset ?? workspaceRevset(workspace) ?? "@";
  const reviewed = useMemo(
    () => new Set(target?.changes.map((c) => c.commitId)),
    [target?.changes],
  );
  const head = target?.changes.at(-1)?.commitId;
  const pr = usePr(workspace);
  const prOn = (r: RowPr) => pr?.number === r.number && (pr.stack || pr.revset === revset);
  const { prOf, known: prsKnown } = useRowPrs(workspace, refreshed);
  const [rows, setRows] = useState<LogRow[]>();
  const [error, setError] = useState<string>();
  /** Index into `commits` of the highlighted commit, and of the other end of a range. */
  const [cursor, setCursor] = useState(0);
  const [anchor, setAnchor] = useState<number>();
  /** The commits jj lists for a range, as of the rows it was asked with. */
  const [chain, setChain] = useState<{ revset: string; ids: Set<string>; rows?: LogRow[] }>();
  const rootRef = useRef<HTMLDivElement>(null);
  /** The commits of a review being loaded, until it's loaded. */
  const [pending, setPending] = useState<Set<string>>();
  /** The commit a drag across rows started on. */
  const drag = useRef<number>(undefined);
  /** The drag has reached a second commit, so it brackets what it's across. A click doesn't. */
  const [dragging, setDragging] = useState(false);

  const commits = useMemo(() => (rows ?? []).flatMap((r, i) => (r.commit ? [i] : [])), [rows]);
  const graph = useMemo(() => graphOf(rows ?? []), [rows]);
  const commitAt = (k: number) => rows?.[commits[k]]?.commit ?? undefined;
  const changeAt = (k: number) => commitAt(k)?.changeId;
  /** The operation the rows are of, so a repo change they already show doesn't reload. */
  const shownOp = useRef("");
  async function reload(keep?: string) {
    const r = await commands.logView(workspace);
    if (r.status === "error") return setError(r.error);
    const { revset, rows, error, operation } = r.data;
    shownOp.current = operation;
    publishLogRevset(workspace, { revset, error });
    setRows(rows);
    const found = rows.filter((l) => l.commit);
    const at = found.findIndex((l) =>
      keep
        ? l.commit?.changeId === keep
        : head
          ? l.commit?.commitId === head
          : l.commit?.workingCopy,
    );
    setCursor(Math.max(at, 0));
    setAnchor(undefined);
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: loads once per workspace
  useEffect(() => {
    reload();
  }, []);
  useRepoChanged(async () => {
    if (busy) return;
    const op = await commands.operation(workspace);
    if (op.status === "ok" && op.data && op.data === shownOp.current) return;
    reload(changeAt(cursor));
  }, workspace);
  // A stack PR stays the workspace's PR only while a row shows its pill.
  useEffect(() => {
    if (!pr?.stack || !rows || !prsKnown) return;
    if (rows.some((r) => r.commit && prOf(r.commit)?.number === pr.number)) return;
    commands.dropStackPr(workspace, pr.number).then((r) => {
      if (r.status === "ok") setPr(workspace, null);
    });
  }, [workspace, pr, rows, prsKnown, prOf]);
  const reloadLatest = useRef(reload);
  reloadLatest.current = reload;
  // biome-ignore lint/correctness/useExhaustiveDependencies: the log is keyed by workspace
  useEffect(() => onLogRevsetChanged(workspace, () => reloadLatest.current()), []);
  useEffect(
    () =>
      onFocusLog(() => {
        rootRef.current?.focus();
      }),
    [],
  );
  /** The keys typed so far while the motions are open. */
  const [typed, setTyped] = useState<string>();
  const groups = useMemo(() => motions(trunk), [trunk]);
  useEffect(
    () =>
      onOpenMotions(() => {
        rootRef.current?.focus();
        setTyped("");
      }),
    [],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: scrolls when the cursor moves
  useEffect(() => {
    rootRef.current?.querySelector(".lr.cursor")?.scrollIntoView({ block: "nearest" });
  }, [cursor, anchor]);

  const [lo, hi] = anchor === undefined ? [cursor, cursor] : [cursor, anchor].sort((a, b) => a - b);

  /** Older to newer, or the one commit, from cursor `c` and anchor `a`. Log order is newest
   * first. */
  function rangeOf(c: number, a = c): string | undefined {
    const newer = changeAt(Math.min(c, a));
    const older = changeAt(Math.max(c, a));
    if (!newer || !older) return undefined;
    return newer === older ? newer : `${older}::${newer}`;
  }
  const picked = () => rangeOf(cursor, anchor);
  const ranged = hi > lo ? picked() : undefined;

  /** A released drag's range, while its commits are asked for. */
  const released = useRef<string>(undefined);
  // The log can interleave branches, so a range holds only the commits `older::newer` lists.
  // A drag asks once, when it's released.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `chain` is what this sets
  useEffect(() => {
    if (!ranged || !rows || dragging || released.current === ranged) return;
    if (chain?.revset === ranged && chain.rows === rows) return;
    let live = true;
    commands.revsetCommits(workspace, ranged).then((r) => {
      if (live && r.status === "ok") setChain({ revset: ranged, ids: new Set(r.data), rows });
    });
    return () => {
      live = false;
    };
  }, [workspace, ranged, rows, dragging]);
  const inChain = ranged && chain?.revset === ranged ? chain.ids : undefined;
  /** The range's ends aren't ancestor and descendant, so `older::newer` is empty. */
  const unrelated = inChain?.size === 0;

  function reviewChain(range: string, ids: Set<string>) {
    if (ids.size === 0) setError(UNRELATED);
    else review(range, ids);
  }

  function reviewPicked() {
    if (unrelated) setError(UNRELATED);
    else review(picked());
  }

  /** Moves the cursor, and the anchor for a range. */
  function move(c: number, a?: number) {
    setCursor(c);
    setAnchor(a);
  }

  // Releasing a drag across commits reviews them; one that stays on its row only moves there.
  useEffect(() => {
    const up = async () => {
      const from = drag.current;
      drag.current = undefined;
      setDragging(false);
      if (from === undefined || from === cursor) return;
      const range = rangeOf(cursor, from);
      if (!range) return;
      if (chain?.revset === range && chain.rows === rows) return reviewChain(range, chain.ids);
      // Asked here rather than by the range's effect, so a reload meanwhile can't drop it.
      released.current = range;
      const asked = rows;
      const r = await commands.revsetCommits(workspace, range);
      if (released.current !== range) return;
      released.current = undefined;
      if (r.status !== "ok") return;
      const ids = new Set(r.data);
      setChain({ revset: range, ids, rows: asked });
      reviewChain(range, ids);
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  });

  /** The commit ids of rows `lo` to `hi` that jj lists for the range. */
  function pickedIds() {
    const ids = new Set<string>();
    for (let c = lo; c <= hi; c++) {
      const id = commitAt(c)?.commitId;
      if (id && (!inChain || inChain.has(id))) ids.add(id);
    }
    return ids;
  }

  const { act, busy, running, note, undo, redo } = useJjActions(
    workspace,
    revset,
    () => reload(changeAt(cursor)),
    setError,
  );

  /** Loads `next` as the review, bracketing its commits dashed until it's loaded: `known`,
   * else the picked rows jj lists for it. */
  async function review(next: string | undefined, known?: Set<string>) {
    if (!next) return;
    setError(undefined);
    const ids = known ?? (next === picked() ? pickedIds() : new Set<string>());
    setPending(ids);
    if (!ids.size) {
      commands.revsetCommits(workspace, next).then((r) => {
        if (r.status === "ok") setPending((p) => (p === ids ? new Set(r.data) : p));
      });
    }
    const failed = await loadReview(workspace, next);
    setPending(undefined);
    if (failed) setError(failed);
  }

  /** Reviews the PR whose pill was clicked: its own revset, or picked from the stack. */
  async function reviewPr(p: RowPr) {
    setError(undefined);
    if (p.own && pr) return review(pr.revset);
    await reviewStackPr(workspace, p.number).catch((e) => setError(errorText(e)));
  }

  async function pinPr(p: RowPr, on: boolean) {
    setError(undefined);
    await pinStackPr(workspace, p.number, on, revset).catch((e) => setError(errorText(e)));
  }

  function edit(c = cursor) {
    const rev = changeAt(c);
    if (rev) act(...editAction(rev));
  }

  /** Moves the cursor to commit `c` unless it's in the range, then opens its menu. */
  function rowMenu(e: React.MouseEvent, c: number) {
    const commit = commitAt(c);
    if (!commit) return;
    if (c < lo || c > hi) move(c);
    openMenu(
      e,
      revRowMenu({
        change: commit.changeId,
        commit: commit.commitId,
        description: commit.description,
        busy,
        review,
        act: (a) => act(...a),
        undo,
        redo,
      }),
    );
  }

  function runMotion(m: Motion) {
    const cursorChange = changeAt(cursor);
    const range = picked();
    if (!cursorChange || !range) return;
    if (m.keys === "s" && unrelated) return setError(UNRELATED);
    const target = { cursor: cursorChange, picked: range };
    const at = rows?.findIndex((r) => r.commit?.workingCopy) ?? -1;
    const here = commits[cursor];
    if (m.ask) setNaming({ motion: m, target, was: rows?.[here]?.commit?.description ?? "" });
    else if (m.pickBookmark) bookmarks.start(m, target);
    else if (m.pickRev && at >= 0 && at !== here && rows) {
      const both = [at, here].sort((a, b) => a - b);
      revs.start(
        m,
        target,
        both.map((row) => ({ row, change: rows[row].commit?.changeId ?? "" })),
      );
    } else runOn(m, target);
  }

  function runOn(m: Motion, target: MotionTarget) {
    const action = m.action(target);
    if (action.kind === "checkpoint") checkpoint();
    else if (action.kind === "undo") undo();
    else if (action.kind === "redo") redo();
    else act(action, m.label, m.follow);
  }

  async function checkpoint() {
    try {
      await unwrap(commands.setCheckpoint(workspace));
      turnsOpen.set(true);
      reloadTurns(workspace);
    } catch (e) {
      setError(errorText(e));
    }
  }

  const hover = useRowHover();
  const bookmarks = useBookmarkPick(workspace, runOn, setError);
  const revs = useRevPick(runOn);
  /** The motion waiting on a name or description typed for it. `was` is the cursor commit's
   * description. */
  const [naming, setNaming] = useState<{ motion: Motion; target: MotionTarget; was: string }>();

  function endNaming(text?: string) {
    if (!naming) return;
    setNaming(undefined);
    rootRef.current?.focus();
    if (!text) return;
    const { motion, target, was } = naming;
    if (motion.ask === "description") runOn(motion, { ...target, description: retitle(was, text) });
    else runOn(motion, { ...target, bookmark: text });
  }

  /** Keys while the motions are open: each narrows them until one runs. Keys no motion takes,
   * like moving and reviewing, act on the log as usual. Returns whether it took the key. */
  function motionKey(e: React.KeyboardEvent, sofar: string): boolean {
    if (e.key === "Escape") setTyped(undefined);
    else if (e.key === "Backspace") setTyped(sofar.slice(0, -1));
    else {
      const next = e.key.length === 1 ? step(groups, sofar, e.key) : undefined;
      if (!next) return false;
      if ("typed" in next) setTyped(next.typed);
      else {
        setTyped(undefined);
        runMotion(next.run);
      }
    }
    e.preventDefault();
    e.stopPropagation();
    return true;
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (bookmarks.key(e) || revs.key(e)) return;
    if (typed !== undefined && !e.metaKey && !e.ctrlKey && motionKey(e, typed)) return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const n = commits.length;
    const d = MOVES[e.key];
    if (d) {
      const range = e.key === "J" || e.key === "K";
      move(Math.min(Math.max(cursor + d, 0), n - 1), range ? (anchor ?? cursor) : undefined);
    } else {
      switch (e.key) {
        case "Enter":
          // A focused button keeps its own Enter.
          if (e.target instanceof HTMLButtonElement) return;
          edit();
          break;
        case "r":
          reviewPicked();
          break;
        case "Escape":
          target?.focus();
          break;
        default:
          return;
      }
    }
    e.preventDefault();
    e.stopPropagation();
  }

  // A drag brackets the commits it's across as it goes, before anything loads.
  /** The review shows turns, which the bracket marks in place of its commits. */
  const onTurn = !dragging && !pending && turns.some((t) => target?.turns?.includes(t.id));
  const tentative = dragging || !!pending;
  const shown = dragging ? pickedIds() : (pending ?? reviewed);
  const inReview = (i: number) => shown.has(rows?.[i]?.commit?.commitId ?? "");
  /** The first and last row of the review, for the bracket beside them. */
  let first = -1;
  let lastIn = -1;
  rows?.forEach((_, i) => {
    if (onTurn || !inReview(i)) return;
    if (first < 0) first = i;
    lastIn = i;
  });

  /** A row's tint: empty yellow over `@` green over reviewed cyan. */
  function tintOf(i: number) {
    const commit = rows?.[i]?.commit;
    if (commit?.empty) return " empty";
    if (commit?.workingCopy) return " at";
    return inReview(i) ? " rv" : "";
  }

  /** A row's graph and its commit's lines. Nothing for a row a reload took away. */
  function rowBody(i: number) {
    const cells = graph.cells[i];
    if (!cells) return null;
    const commit = rows?.[i]?.commit;
    const rp = commit ? prOf(commit) : undefined;
    return (
      <>
        <LogGraph cells={cells} width={graph.width} />
        {commit && (
          <CommitLines
            c={commit}
            lead={commit.workingCopy && <TurnsToggle turns={turns} />}
            pill={
              rp && (
                <PrPill
                  workspace={workspace}
                  pr={rp}
                  on={prOn(rp)}
                  pinned={prOn(rp) && !!pr?.pinned}
                  onReview={() => reviewPr(rp)}
                  onPin={rp.own ? undefined : (on) => pinPr(rp, on)}
                />
              )
            }
          />
        )}
      </>
    );
  }

  const commitIndex = new Map(commits.map((row, k) => [row, k]));

  return (
    <div
      ref={rootRef}
      className="revisions"
      // biome-ignore lint/a11y/noNoninteractiveTabindex: takes the log's keys
      tabIndex={0}
      onKeyDown={onKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setTyped(undefined);
      }}
    >
      {running && <RunningRow action={running} />}
      {error && (
        <div className="review-error log-error">
          <span>{error}</span>
          <IconButton icon="close" label="Clear" onClick={() => setError(undefined)} />
        </div>
      )}
      {!rows && !error && <div className="placeholder">Loading</div>}
      <div className="lr-list" onScroll={hover.leave}>
        {rows?.map((row, i) => {
          const c = commitIndex.get(i) ?? -1;
          const commit = row.commit;
          const inRange = c >= lo && c <= hi && (!inChain || inChain.has(commit?.commitId ?? ""));
          const tint = tintOf(i);
          const edge = first >= 0 && i >= first && i <= lastIn;
          return (
            <Fragment key={commit?.commitId ?? `g${i}`}>
              {/* biome-ignore lint/a11y/useKeyWithMouseEvents: hovering only shows the row wider */}
              <div
                className={`lr${tint}${commit ? " pickable" : ""}${inRange ? " picked" : ""}${c === cursor ? " cursor" : ""}`}
                onMouseDown={(e) => {
                  hover.leave();
                  // A right-click picks in the row's menu, keeping a range it's in.
                  if (c < 0 || e.button !== 0) return;
                  if (!e.shiftKey) {
                    drag.current = c;
                  }
                  move(c, e.shiftKey ? (anchor ?? cursor) : undefined);
                }}
                onMouseOver={(e) => {
                  if (drag.current !== undefined || !commit) return;
                  // Only a cut-off description or bookmark list shows the row wider.
                  const part = (e.target as HTMLElement).closest<HTMLElement>(
                    ".lr-desc, .lr-books",
                  );
                  if (part && part.scrollWidth > part.clientWidth) hover.enter(i, e.currentTarget);
                  else hover.leave();
                }}
                onMouseEnter={() => {
                  if (drag.current === undefined || c < 0) return;
                  if (c !== drag.current) setDragging(true);
                  move(c, drag.current);
                }}
                onMouseLeave={hover.leave}
                onDoubleClick={() => c >= 0 && edit(c)}
                onContextMenu={(e) => c >= 0 && rowMenu(e, c)}
              >
                {edge && (
                  <span
                    className={`lr-bk${i === first ? " top" : ""}${i === lastIn ? " bot" : ""}${tentative ? " loading" : ""}`}
                  />
                )}
                {commit && (
                  <button
                    type="button"
                    className="lr-rail"
                    aria-label="Review"
                    tabIndex={-1}
                    onMouseDown={(e) => {
                      hover.leave();
                      // A drag from the rail brackets commits as one from the row does.
                      e.stopPropagation();
                      if (e.button !== 0) return;
                      drag.current = c;
                      move(c);
                    }}
                    onDoubleClick={(e) => e.stopPropagation()}
                    onClick={() => {
                      move(c);
                      // Off a turn, `@`'s rail goes back to its whole diff.
                      if (onTurn && commit.workingCopy) target?.view({ kind: "combined" });
                      else review(commit.changeId);
                    }}
                  />
                )}
                {rowBody(i)}
              </div>
              {commit?.workingCopy && (
                <TurnRows
                  workspace={workspace}
                  turns={turns}
                  shown={onTurn ? target?.turns : undefined}
                  below={graph.cells[i].at(-1) ?? []}
                  width={graph.width}
                  onError={setError}
                />
              )}
            </Fragment>
          );
        })}
      </div>
      {hover.hover && !dragging && graph.cells[hover.hover.row] && (
        <RevisionPopover hover={hover.hover}>{rowBody(hover.hover.row)}</RevisionPopover>
      )}
      {typed !== undefined && <MotionPopup groups={narrow(groups, typed)} typed={typed} />}
      {naming && (
        <NamePopup
          motion={naming.motion}
          initial={naming.motion.ask === "description" ? title(naming.was) : ""}
          onName={endNaming}
          onCancel={() => endNaming()}
        />
      )}
      {bookmarks.pick && <BookmarkPopup pick={bookmarks.pick} onPick={bookmarks.choose} />}
      {revs.pick && (
        <RevPickPopup
          pick={revs.pick}
          rowClass={(i) => tintOf(i).trim()}
          rowBody={rowBody}
          onPick={revs.choose}
        />
      )}
      <RevisionsFooter note={note} loading={!!pending} />
    </div>
  );
}
