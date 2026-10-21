import { useEffect, useMemo, useRef, useState } from "react";
import type { LogRow } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands, errorText } from "../ipc";
import { editAction, revRowMenu } from "../review/RevisionsPanelMenu";
import { workspaceRevset } from "../review/revset";
import { useReviewTarget } from "../review/target";
import { openMenu } from "../ui/contextMenu";
import { reviewStackPr } from "../workspaces/stack";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { CommitLines } from "./CommitLines";
import { graphOf } from "./graphCells";
import { LogGraph } from "./LogGraph";
import { loadReview, onFocusLog, onOpenMotions } from "./loadReview";
import { onLogRevsetChanged, publishLogRevset } from "./logRevset";
import { MotionPopup } from "./MotionPopup";
import { type Motion, motions, narrow, step } from "./motions";
import { PrPill } from "./PrPill";
import { RevisionsFooter } from "./RevisionsFooter";
import { useJjActions } from "./useJjActions";
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
 * a bracket, `@` green over them, and each open PR's head has its pill. Keys act on the
 * highlighted commit; dragging across commits reviews them, and double-clicking one edits it. */
export function RevisionsLog({ workspace, refreshed, trunk }: Props) {
  const target = useReviewTarget(workspace);
  const revset = target?.revset ?? workspaceRevset(workspace) ?? "@";
  const reviewed = useMemo(
    () => new Set(target?.changes.map((c) => c.commitId)),
    [target?.changes],
  );
  const head = target?.changes.at(-1)?.commitId;
  const pr = usePr(workspace);
  const prOn = (r: RowPr) => pr?.number === r.number && pr.revset === revset;
  const prOf = useRowPrs(workspace, refreshed);
  const [rows, setRows] = useState<LogRow[]>();
  const [error, setError] = useState<string>();
  /** Index into `commits` of the highlighted commit, and of the other end of a range. */
  const [cursor, setCursor] = useState(0);
  const [anchor, setAnchor] = useState<number>();
  /** The commit ids jj lists for a picked range. */
  const [chain, setChain] = useState<{ revset: string; ids: Set<string> }>();
  const rootRef = useRef<HTMLDivElement>(null);
  /** The commits of a review being loaded, until it's loaded. */
  const [pending, setPending] = useState<Set<string>>();
  /** The commit a drag across rows started on. */
  const drag = useRef<number>(undefined);
  const [dragging, setDragging] = useState(false);

  const commits = useMemo(() => (rows ?? []).flatMap((r, i) => (r.commit ? [i] : [])), [rows]);
  const graph = useMemo(() => graphOf(rows ?? []), [rows]);
  const commitAt = (k: number) => rows?.[commits[k]]?.commit ?? undefined;
  const changeAt = (k: number) => commitAt(k)?.changeId;
  async function reload(keep?: string) {
    const r = await commands.logView(workspace);
    if (r.status === "error") return setError(r.error);
    const { revset, rows, error } = r.data;
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
  useRepoChanged(() => {
    if (!busy) reload(changeAt(cursor));
  });
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

  // The log can interleave branches, so a range holds only the commits `older::newer` lists.
  useEffect(() => {
    if (!ranged || !rows) return;
    let live = true;
    commands.revsetCommits(workspace, ranged).then((r) => {
      if (live && r.status === "ok") setChain({ revset: ranged, ids: new Set(r.data) });
    });
    return () => {
      live = false;
    };
  }, [workspace, ranged, rows]);
  const inChain = ranged && chain?.revset === ranged ? chain.ids : undefined;
  /** The range's ends aren't ancestor and descendant, so `older::newer` is empty. */
  const unrelated = inChain?.size === 0;

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
      const r = await commands.revsetCommits(workspace, range);
      if (r.status === "ok" && r.data.length === 0) setError(UNRELATED);
      else review(range);
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

  /** Loads `next` as the review, bracketing its commits dashed until it's loaded. */
  async function review(next: string | undefined) {
    if (!next) return;
    setError(undefined);
    const ids = next === picked() ? pickedIds() : new Set<string>();
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
    const action = m.action({ cursor: cursorChange, picked: range });
    if (action.kind === "undo") undo();
    else if (action.kind === "redo") redo();
    else act(action, m.label, m.follow);
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
  const tentative = dragging || !!pending;
  const shown = dragging ? pickedIds() : (pending ?? reviewed);
  const inReview = (i: number) => shown.has(rows?.[i]?.commit?.commitId ?? "");
  /** The first and last row of the review, for the bracket beside them. */
  let first = -1;
  let lastIn = -1;
  rows?.forEach((_, i) => {
    if (!inReview(i)) return;
    if (first < 0) first = i;
    lastIn = i;
  });

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
      {error && <div className="review-error">{error}</div>}
      {!rows && !error && <div className="placeholder">Loading</div>}
      <div className="lr-list">
        {rows?.map((row, i) => {
          const c = commitIndex.get(i) ?? -1;
          const commit = row.commit;
          const inRange = c >= lo && c <= hi && (!inChain || inChain.has(commit?.commitId ?? ""));
          const tint = commit?.workingCopy ? " at" : inReview(i) ? " rv" : "";
          const edge = first >= 0 && i >= first && i <= lastIn;
          const rp = commit ? prOf(commit) : undefined;
          return (
            <div
              key={commit?.commitId ?? `g${i}`}
              className={`lr${tint}${commit ? " pickable" : ""}${inRange ? " picked" : ""}${c === cursor ? " cursor" : ""}`}
              onMouseDown={(e) => {
                // A right-click picks in the row's menu, keeping a range it's in.
                if (c < 0 || e.button !== 0) return;
                if (!e.shiftKey) {
                  drag.current = c;
                  setDragging(true);
                }
                move(c, e.shiftKey ? (anchor ?? cursor) : undefined);
              }}
              onMouseEnter={() => {
                if (drag.current !== undefined && c >= 0) move(c, drag.current);
              }}
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
                  onMouseDown={(e) => e.stopPropagation()}
                  onDoubleClick={(e) => e.stopPropagation()}
                  onClick={() => {
                    move(c);
                    review(commit.changeId);
                  }}
                />
              )}
              <LogGraph cells={graph.cells[i]} width={graph.width} />
              {commit && (
                <CommitLines
                  c={commit}
                  pill={
                    rp && (
                      <PrPill
                        workspace={workspace}
                        pr={rp}
                        on={prOn(rp)}
                        onReview={() => reviewPr(rp)}
                      />
                    )
                  }
                />
              )}
            </div>
          );
        })}
      </div>
      {typed !== undefined && <MotionPopup groups={narrow(groups, typed)} typed={typed} />}
      <RevisionsFooter running={running} note={note} loading={!!pending} />
    </div>
  );
}
