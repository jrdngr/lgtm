import { useLayoutEffect, useRef, useState } from "react";
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
    <span className={change.description ? "rev-desc" : "rev-desc empty"}>
      {change.description || "no description"}
    </span>
  );
}

interface Props {
  revset: string;
  /** The review's changes, oldest first. */
  changes: ChangeDto[];
}

/** What's reviewed, colored as jj colors it. One commit shows as `@ id description`, with as
 * much of the id as fits up to `MAX_ID`. A range shows `oldest → newest`, the newest
 * description, and how many changes it has. */
export function RevLabel({ revset, changes }: Props) {
  const ref = useRef<HTMLSpanElement>(null);
  const head = changes.at(-1);
  const single = changes.length === 1 || revset === "@";
  const full = head ? Math.max(head.unique, MAX_ID) : 0;
  const [length, setLength] = useState(full);

  // Start from the longest id whenever the label or the room for it changes, then shrink.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const reset = () => setLength(full);
    reset();
    const observer = new ResizeObserver(reset);
    observer.observe(el.closest(".toolbar") ?? el);
    return () => observer.disconnect();
  }, [full]);

  // The id gives up characters before the description is cut off.
  useLayoutEffect(() => {
    const desc = ref.current?.querySelector(".rev-desc");
    const cut = desc && desc.scrollWidth > desc.clientWidth;
    if (single && cut && head && length > head.unique) setLength(length - 1);
  });

  if (!head) return <span className="rev-label">{revset}</span>;
  if (single) {
    return (
      <span ref={ref} className="rev-label">
        <Icon name="commit" size={14} />
        {head.workingCopy && <span className="rev-at">@</span>}
        <Id id={head.changeId} unique={head.unique} length={length} />
        <Desc change={head} />
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
      <span className="rev-count">{changes.length} changes</span>
    </span>
  );
}
