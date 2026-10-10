import { useEffect, useState } from "react";
import type { LogLine } from "../bindings";
import { commands } from "../ipc";
import { parseAnsi } from "./ansi";

interface Props {
  onPick(revset: string): void;
  onClose(): void;
}

/** `jj log` as the user has it configured. Click a commit, shift-click to extend to a range,
 * then Review (or double-click a commit to review just it). */
export function CommitPicker({ onPick, onClose }: Props) {
  const [lines, setLines] = useState<LogLine[]>();
  const [error, setError] = useState<string>();
  const [range, setRange] = useState<[number, number]>();

  useEffect(() => {
    commands.logGraph().then((r) => {
      if (r.status === "ok") setLines(r.data);
      else setError(r.error);
    });
  }, []);

  // Each commit's lines run until the next commit starts.
  const owner: number[] = [];
  let at = -1;
  for (const [i, l] of (lines ?? []).entries()) {
    if (l.commitId) at = i;
    owner.push(at);
  }

  function revset([a, b]: [number, number]): string | undefined {
    const newer = lines?.[Math.min(a, b)]?.changeId;
    const older = lines?.[Math.max(a, b)]?.changeId;
    if (!newer || !older) return undefined;
    return newer === older ? newer : `${older}::${newer}`;
  }

  function click(e: React.MouseEvent, i: number) {
    const start = owner[i];
    if (start < 0) return;
    setRange(e.shiftKey && range ? [range[0], start] : [start, start]);
  }

  const picked = range && revset(range);
  const lo = range ? Math.min(...range) : -1;
  const hi = range ? Math.max(...range) : -2;

  return (
    <>
      <div className="menu-backdrop" onMouseDown={onClose} />
      <div
        className="commit-picker"
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") onClose();
          if (e.key === "Enter" && picked) onPick(picked);
        }}
        // biome-ignore lint/a11y/noNoninteractiveTabindex: takes Escape and Enter
        tabIndex={0}
        ref={(el) => el?.focus({ preventScroll: true })}
      >
        <div className="picker-bar">
          <span className="picker-hint">Click a commit, shift-click for a range</span>
          <span className="spacer" />
          <code>{picked ?? ""}</code>
          <button type="button" disabled={!picked} onClick={() => picked && onPick(picked)}>
            Review
          </button>
        </div>
        {error && <div className="review-error">{error}</div>}
        {!lines && !error && <div className="placeholder">Loading</div>}
        <pre className="log-lines">
          {lines?.map((l, i) => {
            const commit = owner[i];
            const inRange = commit >= 0 && commit >= lo && commit <= hi;
            return (
              <div
                // biome-ignore lint/suspicious/noArrayIndexKey: lines have no identity
                key={i}
                className={`log-line${commit >= 0 ? " pickable" : ""}${inRange ? " picked" : ""}`}
                onMouseDown={(e) => click(e, i)}
                onDoubleClick={() => {
                  const r = revset([commit, commit]);
                  if (r) onPick(r);
                }}
              >
                {parseAnsi(l.text).map((s, j) => (
                  <span
                    // biome-ignore lint/suspicious/noArrayIndexKey: spans have no identity
                    key={j}
                    style={{ color: s.color, fontWeight: s.bold ? "bold" : undefined }}
                  >
                    {s.text}
                  </span>
                ))}
                {"\n"}
              </div>
            );
          })}
        </pre>
      </div>
    </>
  );
}
