import { type ReactNode, useLayoutEffect, useRef, useState } from "react";
import type { ChangeDto } from "../bindings";
import { Icon } from "../ui/icons";

/** Change ids show at least their unique prefix, and up to this many characters with room. */
const MAX_ID = 5;
/** A range's ends show this many characters, or their unique prefix if longer. */
const RANGE_ID = 3;

function Id({ id, unique, length }: { id: string; unique: number; length: number }) {
  return (
    <span className="rev-id">
      <span className="rev-unique">{id.slice(0, unique)}</span>
      {id.slice(unique, Math.max(unique, length))}
    </span>
  );
}

function Desc({ change }: { change: ChangeDto }) {
  return (
    <span className={change.description || change.empty ? "rev-desc" : "rev-desc empty"}>
      {change.description || "no description"}
    </span>
  );
}

/** The newest bookmark in `changes`, and whether its remote lacks it or is behind. */
function newestBookmark(changes: ChangeDto[]) {
  const marked = [...changes].reverse().find((c) => c.bookmarks.length);
  const name = marked?.bookmarks[0];
  return name && { name: name.replace(/\*$/, ""), unpushed: name.endsWith("*") };
}

/** What the label shows past the id and description, highest priority first. */
function extras(revset: string, changes: ChangeDto[], single: boolean): ReactNode[] {
  const head = changes[changes.length - 1];
  const out: ReactNode[] = [];
  const flags = [
    changes.some((c) => c.conflict) && "conflict",
    changes.some((c) => c.divergent) && "divergent",
    single && head.empty && "empty",
  ].filter((f) => f !== false);
  if (flags.length) {
    out.push(
      <span key="flags" className="rev-flags">
        {flags.map((f) => (
          <span key={f} className={`rev-flag ${f}`}>
            {f}
          </span>
        ))}
      </span>,
    );
  }
  const bookmark = newestBookmark(changes);
  if (bookmark) {
    out.push(
      <span key="bm" className="rev-bookmark">
        {bookmark.name}
        {bookmark.unpushed && <span className="rev-unpushed">*</span>}
      </span>,
    );
  }
  if (!single) {
    out.push(
      <span key="count" className="rev-count">
        {changes.length} changes
      </span>,
    );
  }
  const custom = revset !== "@" && !(single && head.changeId.startsWith(revset));
  if (custom) {
    out.push(
      <span key="revset" className="rev-revset">
        {revset}
      </span>,
    );
  }
  return out;
}

interface Props {
  revset: string;
  /** The review's changes, oldest first. */
  changes: ChangeDto[];
}

/** What's reviewed, colored as jj colors it. One commit shows as `@ id description`, with as
 * much of the id as fits up to `MAX_ID`. A range shows `oldest → newest` and the newest
 * description. Then come state flags, the newest bookmark, a range's change count, and a
 * custom revset, each dropped, last first, while the description is cut off. */
export function RevLabel({ revset, changes }: Props) {
  const ref = useRef<HTMLSpanElement>(null);
  const head = changes.at(-1);
  const single = changes.length === 1 || revset === "@";
  const full = head ? Math.max(head.unique, MAX_ID) : 0;
  const more = head ? extras(revset, changes, single) : [];
  const [length, setLength] = useState(full);
  const [kept, setKept] = useState(more.length);
  const shown = `${revset}\n${changes.map((c) => c.commitId + c.bookmarks).join()}`;

  // Start from everything whenever the label or the room for it changes, then shrink.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `shown` is what the label holds
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const reset = () => {
      setLength(full);
      setKept(more.length);
    };
    reset();
    const observer = new ResizeObserver(reset);
    observer.observe(el.closest(".toolbar") ?? el);
    return () => observer.disconnect();
  }, [full, more.length, shown]);

  // While the description is cut off, the id gives up characters, then the extras go.
  useLayoutEffect(() => {
    const desc = ref.current?.querySelector(".rev-desc");
    if (!head || !desc || desc.scrollWidth <= desc.clientWidth) return;
    if (single && length > head.unique) setLength(length - 1);
    else if (kept > 0) setKept(kept - 1);
  });

  if (!head) return <span className="rev-label">{revset}</span>;
  if (single) {
    return (
      <span ref={ref} className="rev-label">
        <Icon name="commit" size={14} />
        {head.workingCopy && <span className="rev-at">@</span>}
        <Id id={head.changeId} unique={head.unique} length={length} />
        <Desc change={head} />
        {more.slice(0, kept)}
      </span>
    );
  }
  const first = changes[0];
  return (
    <span ref={ref} className="rev-label">
      <Icon name="gitTree" size={14} />
      <Id id={first.changeId} unique={first.unique} length={RANGE_ID} />
      <span className="rev-arrow">→</span>
      <Id id={head.changeId} unique={head.unique} length={RANGE_ID} />
      <Desc change={head} />
      {more.slice(0, kept)}
    </span>
  );
}
