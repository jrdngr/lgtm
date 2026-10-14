import type { Proposed } from "../bindings";
import { type DiffLine, lineCounts, lineDiff } from "./lineDiff";

const split = (path: string) => {
  const at = path.lastIndexOf("/") + 1;
  return [path.slice(0, at), path.slice(at)];
};

/** A proposed file change: its path and line counts, then its hunks unless `folded`. `side`
 * goes at the right of the header. */
export function FileDiff({
  p,
  folded,
  side,
  onToggle,
}: {
  p: Proposed;
  folded?: boolean;
  side?: React.ReactNode;
  onToggle?(): void;
}) {
  const hunks = lineDiff(p.before, p.after);
  const { added, removed } = lineCounts(hunks);
  const [dir, name] = split(p.path);
  const head = (
    <>
      {onToggle && <span className="pd-chev">{folded ? "▸" : "▾"}</span>}
      <span className="pd-path">
        <span className="pd-dir">{dir}</span>
        {name}
      </span>
      <span className="pd-count">
        <span className="pd-a">+{added}</span> <span className="pd-d">−{removed}</span>
      </span>
    </>
  );
  return (
    <div className="pd-file">
      <div className="pd-head">
        {onToggle ? (
          <button type="button" className="pd-toggle" aria-expanded={!folded} onClick={onToggle}>
            {head}
          </button>
        ) : (
          head
        )}
        <span className="spacer" />
        {side}
      </div>
      {!folded && (
        <div className="pd-body">
          {hunks.map((h, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: hunks never reorder
            <Hunk key={i} lines={h} first={i === 0} />
          ))}
        </div>
      )}
    </div>
  );
}

function Hunk({ lines, first }: { lines: DiffLine[]; first: boolean }) {
  return (
    <>
      {!first && <div className="pd-gap">⋯</div>}
      {lines.map((l, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: lines never reorder
        <div key={i} className={`pd-line ${l.kind}`}>
          <span className="pd-no">{l.line}</span>
          <span className="pd-sign">{{ ctx: " ", add: "+", del: "−" }[l.kind]}</span>
          <span className="pd-text">{l.text}</span>
        </div>
      ))}
    </>
  );
}
