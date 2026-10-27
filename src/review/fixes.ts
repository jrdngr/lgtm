import type { CommentDto, FixDto, ThreadDto, View } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { threadLabel } from "../model/stages";
import { targetOf } from "./target";

/** The fix a view shows, if it's the Fix view. */
export const viewedFix = (view: View | undefined) => (view?.kind === "fix" ? view.fix : undefined);

/** The threads fix `fix` is linked to. */
export const fixedThreads = (threads: ThreadDto[], fix: number) =>
  threads.filter((t) => t.fixes.some((f) => f.id === fix));

/** Only the comments on the threads fix `fix` is linked to, or all of them outside the Fix
 * view. */
export function fixComments(comments: CommentDto[], threads: ThreadDto[], fix?: number) {
  if (fix === undefined) return comments;
  const ids = new Set(fixedThreads(threads, fix).map((t) => t.id));
  return comments.filter((c) => ids.has(c.id));
}

/** A fix's time, as `14:02`. */
export const fixTime = (f: FixDto) =>
  new Date(f.at ?? 0).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/** The Fix view's picker: each fix of the thread fix `fix` is on, named by the thread and, when
 * it has more than one, the time. */
export function fixChoices(threads: ThreadDto[], fix: number): [number, string][] {
  const t = fixedThreads(threads, fix)[0];
  if (!t) return [];
  const name = threadLabel(t);
  if (t.fixes.length === 1) return [[t.fixes[0].id, name]];
  return t.fixes.map((f) => [f.id, `${name} · ${fixTime(f)}`]);
}

/** Shows fix `fix` in the workspace's review pane. */
export function openFix(workspace: WorkspaceName, fix: number) {
  targetOf(workspace)?.view({ kind: "fix", fix });
}
