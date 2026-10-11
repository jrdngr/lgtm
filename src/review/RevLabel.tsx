import { useLayoutEffect, useRef, useState } from "react";
import type { ChangeDto } from "../bindings";

/** Change ids show at least their unique prefix, and up to this many characters with room. */
const MAX_ID = 5;

function Id({ id, unique, length }: { id: string; unique: number; length: number }) {
  return (
    <span className="rev-id">
      <span className="rev-unique">{id.slice(0, unique)}</span>
      {id.slice(unique, Math.max(unique, length))}
    </span>
  );
}

/** A revset with each change id it names cut to its unique prefix. */
function Revset({ revset, changes }: { revset: string; changes: ChangeDto[] }) {
  const parts = revset.split(/([^a-z0-9]+)/);
  return (
    <span className="rev-set">
      {parts.map((p, i) => {
        const c = p && changes.find((ch) => ch.changeId.startsWith(p) && p.length >= ch.unique);
        return c ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
          <Id key={i} id={c.changeId} unique={c.unique} length={c.unique} />
        ) : (
          p
        );
      })}
    </span>
  );
}

interface Props {
  revset: string;
  /** The review's changes, oldest first. */
  changes: ChangeDto[];
}

/** What's reviewed, colored as jj colors it. One commit shows as `@ id description`, with as
 * much of the id as fits up to `MAX_ID`. A range shows its revset with short ids. */
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
    if (cut && head && length > head.unique) setLength(length - 1);
  });

  if (!head) return <span className="rev-label">{revset}</span>;
  return (
    <span ref={ref} className="rev-label">
      {single ? (
        <>
          {head.workingCopy && <span className="rev-at">@</span>}
          <Id id={head.changeId} unique={head.unique} length={length} />
          <span className={head.description ? "rev-desc" : "rev-desc empty"}>
            {head.description || "(no description set)"}
          </span>
        </>
      ) : (
        <Revset revset={revset} changes={changes} />
      )}
    </span>
  );
}
