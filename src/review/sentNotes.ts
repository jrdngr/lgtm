import { useSyncExternalStore } from "react";
import type { CommentDto } from "../bindings";

/** Each note sent to a terminal, by id, with when it was last changed as sent. An edit after
 * sending makes it unsent again. */
const sent = new Map<number, number | null>();
const listeners = new Set<() => void>();
let version = 0;

const stamp = (c: CommentDto) => c.updatedAt ?? c.createdAt;

export function markSent(notes: CommentDto[]) {
  for (const c of notes) sent.set(c.id, stamp(c));
  version++;
  for (const l of listeners) l();
}

export const isSent = (c: CommentDto) => sent.has(c.id) && sent.get(c.id) === stamp(c);

/** Rerenders when notes are marked sent. */
export function useSentNotes() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => version,
  );
}
