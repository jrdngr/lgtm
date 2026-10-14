import { useEffect, useRef, useState } from "react";
import type { JjAction, LogLine } from "../bindings";
import { commands } from "../ipc";
import { openMenu } from "../ui/contextMenu";
import type { WorkspaceName } from "../workspaces/context";
import { AnsiText } from "./AnsiText";
import { RevisionsPanelActions } from "./RevisionsPanelActions";
import { RevisionsPanelFooter } from "./RevisionsPanelFooter";
import { presets, REV_ACTIONS, reviewAfter, revRowMenu } from "./RevisionsPanelMenu";
import "./RevisionsPanel.css";

interface Props {
  workspace: WorkspaceName;
  /** The review's revset. */
  revset: string;
  /** The review's newest commit, where the cursor starts. */
  head?: string;
  /** Loads a revset as the review. Returns the error message when it fails. */
  onLoad(revset: string): Promise<string | undefined>;
  onClose(): void;
}

/** Cursor moves. Shifted keys extend the range from the anchor. */
const MOVES: Record<string, number> = { j: 1, ArrowDown: 1, J: 1, k: -1, ArrowUp: -1, K: -1 };

const UNRELATED = "Not one line of ancestry";

/** Line index of each commit's first line, newest first, and the commit each line belongs to
 * (-1 before the first). */
function layout(lines: LogLine[]) {
  const commits: number[] = [];
  const owner = lines.map((l, i) => {
    if (l.commitId) commits.push(i);
    return commits.length - 1;
  });
  return { commits, owner };
}

/** `jj log` as the user has it configured, to pick what to review and to edit, add, and squash
 * commits. Keys and the cursor row's buttons act on the highlighted commit. */
export function RevisionsPanel({ workspace, revset, head, onLoad, onClose }: Props) {
  const [lines, setLines] = useState<LogLine[]>();
  const [error, setError] = useState<string>();
  /** Index into `commits` of the highlighted commit, and of the other end of a range. */
  const [cursor, setCursor] = useState(0);
  const [anchor, setAnchor] = useState<number>();
  const [typed, setTyped] = useState(revset);
  const [last, setLast] = useState<string>();
  /** How many jj operations the last action took, so undo and redo take back all of them. */
  const [steps, setSteps] = useState(1);
  const [busy, setBusy] = useState(false);
  const [trunk, setTrunk] = useState<string | null>(null);
  /** The commit ids jj lists for a picked range. */
  const [chain, setChain] = useState<{ revset: string; ids: Set<string> }>();
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const { commits, owner } = layout(lines ?? []);
  const changeAt = (k: number) => lines?.[commits[k]]?.changeId ?? undefined;

  async function reload(keep?: string) {
    const r = await commands.logGraph(workspace);
    if (r.status === "error") return setError(r.error);
    setLines(r.data);
    const starts = r.data.filter((l) => l.commitId);
    const at = starts.findIndex((l) => (keep ? l.changeId === keep : l.commitId === head));
    setCursor(Math.max(at, 0));
    setAnchor(undefined);
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once on open
  useEffect(() => {
    reload();
    commands.trunkBookmark(workspace).then((r) => r.status === "ok" && setTrunk(r.data));
    rootRef.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    rootRef.current?.querySelector(".log-line.cursor")?.scrollIntoView({ block: "nearest" });
  });

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
    if (!ranged || !lines) return;
    let live = true;
    commands.revsetCommits(workspace, ranged).then((r) => {
      if (live && r.status === "ok") setChain({ revset: ranged, ids: new Set(r.data) });
    });
    return () => {
      live = false;
    };
  }, [workspace, ranged, lines]);
  const inChain = ranged && chain?.revset === ranged ? chain.ids : undefined;
  /** The range's ends aren't ancestor and descendant, so `older::newer` is empty. */
  const unrelated = inChain?.size === 0;

  function reviewPicked() {
    if (unrelated) setError(UNRELATED);
    else review(picked());
  }

  /** Moves the cursor, and the anchor for a range, and shows the pick in the field. */
  function move(c: number, a?: number) {
    setCursor(c);
    setAnchor(a);
    setTyped(rangeOf(c, a) ?? typed);
  }

  /** What the review button loads: the pick while the field shows it, else the field. */
  function reviewLabel() {
    if (typed.trim() === "@") return "Review @";
    if (typed !== picked()) return "Review revset";
    if (hi > lo) return unrelated ? UNRELATED : `Review ${inChain?.size ?? hi - lo + 1} changes`;
    const line = lines?.[commits[lo]];
    return `Review ${line?.workingCopy ? "@" : line?.changeId}`;
  }

  async function review(target: string | undefined) {
    if (!target) return;
    const failed = await onLoad(target);
    if (failed) setError(failed);
    else onClose();
  }

  async function act(action: JjAction, label: string, next?: string) {
    if (busy) return;
    setBusy(true);
    try {
      setError(undefined);
      const r = await commands.jjAct(workspace, action);
      if (r.status === "error") return setError(r.error);
      setLast(label);
      if (action.kind !== "undo" && action.kind !== "redo") setSteps(r.data);
      await reload(changeAt(cursor));
      // Undo, redo and squash keep the review's revset unless it no longer resolves.
      const failed = await reviewAfter(onLoad, revset, next);
      if (failed) setError(failed);
    } finally {
      setBusy(false);
    }
  }

  function runKey(key: string) {
    const rev = changeAt(cursor);
    if (rev) act(...REV_ACTIONS[key](rev));
  }

  const undo = () => act({ kind: "undo", steps }, "Undid the last action");
  const redo = () => act({ kind: "redo", steps }, "Redid the action");

  /** Moves the cursor to commit `c` unless it's in the range, then opens its menu. */
  function rowMenu(e: React.MouseEvent, c: number) {
    const line = lines?.[commits[c]];
    if (!line?.changeId || !line.commitId) return;
    const outside = c < lo || c > hi;
    if (outside) move(c);
    const commit = line.commitId;
    openMenu(
      e,
      revRowMenu({
        change: line.changeId,
        commit,
        description: line.description ?? undefined,
        alone: outside || lo === hi,
        busy,
        review,
        act: (a) => act(...a),
        undo,
        redo,
      }),
    );
  }

  function onKeyDown(e: React.KeyboardEvent) {
    e.stopPropagation();
    if (e.target === inputRef.current) {
      if (e.key === "Escape") rootRef.current?.focus();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const n = commits.length;
    const d = MOVES[e.key];
    if (d) {
      const range = e.key === "J" || e.key === "K";
      move(Math.min(Math.max(cursor + d, 0), n - 1), range ? (anchor ?? cursor) : undefined);
    } else if (e.key in REV_ACTIONS) {
      runKey(e.key);
    } else {
      switch (e.key) {
        case "Enter":
          // A focused button keeps its own Enter.
          if (e.target instanceof HTMLButtonElement) return;
          reviewPicked();
          break;
        case "Escape":
          onClose();
          break;
        case "/":
          inputRef.current?.focus();
          inputRef.current?.select();
          break;
        case "u":
          undo();
          break;
        case "U":
          redo();
          break;
        default:
          return;
      }
    }
    e.preventDefault();
  }

  return (
    <>
      <div className="rev-backdrop" onMouseDown={onClose} />
      <div
        ref={rootRef}
        className="commit-picker"
        // biome-ignore lint/a11y/noNoninteractiveTabindex: takes the panel's keys
        tabIndex={0}
        onKeyDown={onKeyDown}
      >
        <form
          className="picker-bar"
          onSubmit={(e) => {
            e.preventDefault();
            review(typed.trim());
          }}
        >
          <label className="revset-field">
            <span>revset</span>
            <input
              ref={inputRef}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              spellCheck={false}
            />
          </label>
          <button type="submit" className="primary" disabled={unrelated && typed === ranged}>
            {reviewLabel()}
          </button>
        </form>
        <div className="picker-presets">
          {presets(trunk).map((p) => (
            <button
              type="button"
              key={p}
              className={p === revset ? "on" : undefined}
              onClick={() => review(p)}
            >
              {p}
            </button>
          ))}
        </div>
        {error && <div className="review-error">{error}</div>}
        {!lines && !error && <div className="placeholder">Loading</div>}
        <pre className="log-lines">
          {lines?.map((l, i) => {
            const c = owner[i];
            const inRange =
              c >= lo && c <= hi && (!inChain || inChain.has(lines[commits[c]].commitId ?? ""));
            const isCursor = c === cursor && !!l.commitId;
            return (
              <div
                // biome-ignore lint/suspicious/noArrayIndexKey: lines have no identity
                key={i}
                className={`log-line${c >= 0 ? " pickable" : ""}${inRange ? " picked" : ""}${isCursor ? " cursor" : ""}`}
                onMouseDown={(e) => {
                  // A right-click picks in the row's menu, keeping a range it's in.
                  if (c < 0 || e.button !== 0) return;
                  move(c, e.shiftKey ? (anchor ?? cursor) : undefined);
                }}
                onDoubleClick={() => review(changeAt(c))}
                onContextMenu={(e) => c >= 0 && rowMenu(e, c)}
              >
                <AnsiText text={l.text} />
                {isCursor && !busy && (
                  <RevisionsPanelActions onReview={reviewPicked} onKey={runKey} />
                )}
                {"\n"}
              </div>
            );
          })}
        </pre>
        <RevisionsPanelFooter last={last} busy={busy} />
      </div>
    </>
  );
}
