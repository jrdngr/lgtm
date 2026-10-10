import { useEffect, useRef, useState } from "react";
import type { JjAction, LogLine } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { AnsiText } from "./AnsiText";

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

const HINTS =
  "↵ review · e edit · n new after · N new before · s squash · u undo · U redo · / revset";

/** `jj log` as the user has it configured, to pick what to review and to edit, add, and squash
 * commits. Keys act on the highlighted commit. */
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
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  /** Line index of each commit's first line, newest first. */
  const commits = (lines ?? []).flatMap((l, i) => (l.commitId ? [i] : []));
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
    rootRef.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    rootRef.current?.querySelector(".log-line.cursor")?.scrollIntoView({ block: "nearest" });
  });

  const [lo, hi] = anchor === undefined ? [cursor, cursor] : [cursor, anchor].sort((a, b) => a - b);

  /** Older to newer, or the one commit. Log order is newest first. */
  function picked(): string | undefined {
    const newer = changeAt(lo);
    const older = changeAt(hi);
    if (!newer || !older) return undefined;
    return newer === older ? newer : `${older}::${newer}`;
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
    setError(undefined);
    const r = await commands.jjAct(workspace, action);
    if (r.status === "error") {
      setError(r.error);
      setBusy(false);
      return;
    }
    setLast(label);
    if (action.kind !== "undo" && action.kind !== "redo") setSteps(r.data);
    const keep = changeAt(cursor);
    await reload(keep);
    // Undo, redo and squash keep the review's revset unless it no longer resolves.
    const failed = await onLoad(next ?? revset);
    if (failed && !next) await onLoad("@");
    else if (failed) setError(failed);
    setBusy(false);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    e.stopPropagation();
    if (e.target === inputRef.current) {
      if (e.key === "Escape") rootRef.current?.focus();
      return;
    }
    const n = commits.length;
    const rev = changeAt(cursor);
    const move = (d: number) => setCursor((c) => Math.min(Math.max(c + d, 0), n - 1));
    switch (e.key) {
      case "j":
      case "ArrowDown":
        setAnchor(undefined);
        move(1);
        break;
      case "k":
      case "ArrowUp":
        setAnchor(undefined);
        move(-1);
        break;
      case "J":
        setAnchor((a) => a ?? cursor);
        move(1);
        break;
      case "K":
        setAnchor((a) => a ?? cursor);
        move(-1);
        break;
      case "Enter":
        review(picked());
        break;
      case "Escape":
        onClose();
        break;
      case "/":
        inputRef.current?.focus();
        inputRef.current?.select();
        break;
      case "e":
        if (rev) act({ kind: "edit", rev }, `Editing ${rev}`, "@");
        break;
      case "n":
        if (rev) act({ kind: "newAfter", rev }, `New commit after ${rev}`, `${rev}::@`);
        break;
      case "N":
        if (rev) act({ kind: "newBefore", rev }, `New commit before ${rev}`, `@::${rev}`);
        break;
      case "s":
        if (rev) act({ kind: "squash", rev }, `Squashed ${rev} into its parent`);
        break;
      case "u":
        act({ kind: "undo", steps }, "Undid the last action");
        break;
      case "U":
        act({ kind: "redo", steps }, "Redid the action");
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  const owner: number[] = [];
  let k = -1;
  for (const l of lines ?? []) {
    if (l.commitId) k++;
    owner.push(k);
  }

  return (
    <>
      <div className="menu-backdrop" onMouseDown={onClose} />
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
          <input
            ref={inputRef}
            className="revset"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            spellCheck={false}
            aria-label="Revset"
          />
          <button type="submit">Load</button>
          <span className="spacer" />
          <code>{picked() ?? ""}</code>
        </form>
        {error && <div className="review-error">{error}</div>}
        {!lines && !error && <div className="placeholder">Loading</div>}
        <pre className="log-lines">
          {lines?.map((l, i) => {
            const c = owner[i];
            const inRange = c >= lo && c <= hi;
            return (
              <div
                // biome-ignore lint/suspicious/noArrayIndexKey: lines have no identity
                key={i}
                className={`log-line${c >= 0 ? " pickable" : ""}${inRange ? " picked" : ""}${c === cursor && l.commitId ? " cursor" : ""}`}
                onMouseDown={(e) => {
                  if (c < 0) return;
                  setAnchor(e.shiftKey ? (anchor ?? cursor) : undefined);
                  setCursor(c);
                }}
                onDoubleClick={() => review(changeAt(c))}
              >
                <AnsiText text={l.text} />
                {"\n"}
              </div>
            );
          })}
        </pre>
        <div className="picker-footer">
          {last && <span className="picker-last">{last} · u to undo</span>}
          <span className="spacer" />
          <span>{busy ? "Working…" : HINTS}</span>
        </div>
      </div>
    </>
  );
}
