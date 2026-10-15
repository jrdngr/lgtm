import { useEffect, useState } from "react";
import type { ContextLine, ReviewDto } from "../bindings";
import { commands, unwrap } from "../ipc";
import { type Region, regionKey } from "./model";

/** Unchanged code clicked open, by `regionKey`. Kept until the review closes, and only for
 * regions whose bounds haven't changed since. */
export function useContextLines(review: ReviewDto | undefined, onError: (e: string) => void) {
  const [lines, setLines] = useState<ReadonlyMap<string, ContextLine[]>>(new Map());
  const id = review?.id;
  useEffect(() => {
    void id;
    setLines(new Map());
  }, [id]);

  async function show(file: number, region: Region) {
    if (!review) return;
    const path = review.files[file]?.path;
    if (path === undefined) return;
    try {
      const got = await unwrap(commands.contextLines(review.id, file, region.from, region.to));
      setLines((m) => new Map(m).set(regionKey(path, region), got));
    } catch (e) {
      onError(String(e));
    }
  }

  return { lines, show };
}
