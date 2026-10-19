import type { ChangeDto, PrDto, WorkingCopyDto } from "../bindings";
import "./ReviewBox.css";

/** The rail shows at most this many commits. */
const MAX_DOTS = 8;

/** A dot per reviewed commit, oldest first, with `@` ringed where it is: on its commit, or
 * past a dashed line when it builds on the review. */
function Rail({ n, at, onTop }: { n: number; at: number; onTop: boolean }) {
  const shown = Math.min(n, MAX_DOTS);
  const ring = at < 0 || n <= MAX_DOTS ? at : Math.round((at * (shown - 1)) / (n - 1));
  const parts = [];
  for (let i = 0; i < shown; i++) {
    if (i > 0) parts.push(<em key={`e${i}`} />);
    parts.push(<i key={i} className={i === ring ? "at" : undefined} />);
  }
  if (onTop) parts.push(<em key="dash" className="dash" />, <i key="at" className="at" />);
  return <span className="rev-rail">{parts}</span>;
}

/** What follows `@`: its place in the review, or that it's outside. */
function atTag(wc: WorkingCopyDto, at: number, n: number) {
  if (wc.place === "outside") return "outside";
  if (wc.place === "onTop") return wc.empty ? "empty, on top" : "on top";
  if (n > 1) return `${at + 1} of ${n}`;
  return wc.empty ? "empty" : undefined;
}

interface Props {
  /** The review's changes, oldest first. */
  changes: ChangeDto[];
  /** The workspace's PR, when the review is its commits. */
  pr?: PrDto | null;
  wc?: WorkingCopyDto;
  stale?: boolean;
}

/** What's reviewed and where `@` is. A PR's review is blue with its number; any other is
 * neutral with its commit count. Red when `@` is outside the review. */
export function ReviewBox({ changes, pr, wc, stale }: Props) {
  const head = changes.at(-1);
  const n = changes.length;
  const at = changes.findIndex((c) => c.workingCopy);
  const outside = wc?.place === "outside";
  const desc = pr ? pr.title : head?.description;
  const flags = [
    changes.some((c) => c.conflict) && "conflict",
    changes.some((c) => c.divergent) && "divergent",
    stale && "stale",
  ].filter((f) => f !== false);
  const tag = wc && atTag(wc, at, n);
  return (
    <span className={`rev-box${pr ? " pr" : ""}${outside ? " out" : ""}`}>
      {pr && <span className="rev-tag pr">#{pr.number}</span>}
      <Rail n={n} at={at} onTop={wc?.place === "onTop"} />
      <span className={desc ? "rev-desc" : "rev-desc empty"}>{desc || "no description"}</span>
      {!pr && n > 1 && <span className="rev-count">{n} commits</span>}
      {flags.map((f) => (
        <span key={f} className={`rev-tag flag ${f}`}>
          {f}
        </span>
      ))}
      {wc && (
        <>
          <span className="rev-sep" />
          <span className={outside ? "rev-at out" : "rev-at"}>@</span>
          {tag && <span className={outside ? "rev-tag out" : "rev-tag at"}>{tag}</span>}
        </>
      )}
    </span>
  );
}
