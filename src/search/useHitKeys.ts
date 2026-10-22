import { useEffect, useRef, useState } from "react";
import type { Hit } from "../bindings";

/** A cursor over `hits` moved with ↑↓, which ↵ opens and ⌥↵ sends, all of them. */
export function useHitKeys(
  hits: Hit[],
  open: (h: Hit) => void,
  send: () => void,
  extra?: (e: React.KeyboardEvent) => boolean,
) {
  const [at, setAt] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: back to the top on new results
  useEffect(() => setAt(0), [hits]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: scrolls when the cursor moves
  useEffect(() => {
    listRef.current?.querySelector("[data-on]")?.scrollIntoView({ block: "nearest" });
  }, [at]);

  function onKeyDown(e: React.KeyboardEvent) {
    if (extra?.(e)) return;
    if (e.target instanceof HTMLInputElement && e.key !== "ArrowDown" && e.key !== "Enter") return;
    if (e.key === "ArrowDown") setAt((a) => Math.min(a + 1, hits.length - 1));
    else if (e.key === "ArrowUp") setAt((a) => Math.max(a - 1, 0));
    else if (e.key === "Enter" && e.altKey) send();
    else if (e.key === "Enter" && !e.metaKey && hits[at]) open(hits[at]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  return { at, setAt, listRef, onKeyDown };
}
