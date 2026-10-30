import { useEffect, useState } from "react";

/** How long a canvas's collapse or its undoing animates. */
const ANIMATE_MS = 550;

/** Whether `key` changed less than `ANIMATE_MS` ago, true from the first render with a new
 * key, since a refit lands in that render. */
export function useAnimating(key: string | undefined): boolean {
  const [settled, setSettled] = useState(key);
  useEffect(() => {
    const t = setTimeout(() => setSettled(key), ANIMATE_MS);
    return () => clearTimeout(t);
  }, [key]);
  return settled !== key;
}
