import { useEffect, useRef, useState } from "react";
import type { ReviewDto } from "../bindings";
import { remembered } from "../core/remembered";
import { type DisplayLine, type FlatFile, markKey } from "./model";

/** Whether the review scrolls to the rows each reload adds. */
export const followOn = remembered("reviewFollow", false, [true, false]);

const NONE = new Set<string>();

/** The changed rows of `after` that `before` didn't have, by file path, kind and text. */
export function freshRows(before: FlatFile[], after: FlatFile[]): Set<string> {
  const sig = (path: string, kind: string, text: string) => `${path}\0${kind}\0${text}`;
  const old = new Map<string, number>();
  for (const f of before) {
    for (const r of f.rows) {
      if (r.kind === "context") continue;
      const k = sig(f.dto.path, r.kind, r.text);
      old.set(k, (old.get(k) ?? 0) + 1);
    }
  }
  const fresh = new Set<string>();
  after.forEach((f, file) => {
    f.rows.forEach((r, row) => {
      if (r.kind === "context") return;
      const k = sig(f.dto.path, r.kind, r.text);
      const left = old.get(k) ?? 0;
      if (left > 0) old.set(k, left - 1);
      else fresh.add(markKey(file, row));
    });
  });
  return fresh;
}

/** The first line showing one of `rows`. */
export function firstLine(lines: DisplayLine[], rows: Set<string>): number | undefined {
  const i = lines.findIndex(
    (l) => l.kind === "rows" && l.rows.some((r) => rows.has(markKey(l.file, r))),
  );
  return i < 0 ? undefined : i;
}

/** While Follow is on, the rows the latest reload of the same view added, scrolled to as
 * they arrive. A new view or revset starts over. */
export function useFollowChanges(
  review: ReviewDto | undefined,
  files: FlatFile[],
  lines: DisplayLine[],
  scrollTo: (to: { line: number; center?: boolean }) => void,
): Set<string> {
  const on = followOn.use();
  const [fresh, setFresh] = useState(NONE);
  const prev = useRef<{ view: string; files: FlatFile[] }>(undefined);
  useEffect(() => {
    if (!on) setFresh(NONE);
  }, [on]);
  const view = review ? `${review.revset}\0${JSON.stringify(review.view)}` : "";
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once per new set of files
  useEffect(() => {
    const before = prev.current;
    prev.current = { view, files };
    if (!on || !before || before.view !== view || before.files === files) return;
    const rows = freshRows(before.files, files);
    if (rows.size === 0) return;
    setFresh(rows);
    const line = firstLine(lines, rows);
    if (line !== undefined) scrollTo({ line, center: true });
  }, [files]);
  return on ? fresh : NONE;
}
