import type { JjAction } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { promptText } from "../ui/prompt";
import { followWorkingCopy } from "./followWorkingCopy";

/** An action on a commit: the jj action, its label, and whether the review follows `@` after
 * it. */
export type RevAction = [JjAction, string, boolean?];

/** Edits `rev`; the review follows `@` there. */
export const editAction = (rev: string): RevAction => [
  { kind: "edit", rev },
  `Editing ${rev}`,
  true,
];

/** Revsets offered for review: `@`, its parent, and everything since trunk, named by trunk's
 * bookmark when it has one. */
export function presets(trunk: string | null) {
  return ["@", "@-", `${trunk ?? "trunk()"}..@`];
}

/** Asks for a revset to review, starting from the current one. */
export const askRevset = (revset: string) =>
  promptText("Review revset", { value: revset, action: "Review" });

/** Reloads the review after a jj action: on `revset` while it still resolves, on `@` once it
 * doesn't. With `follow`, the review then follows `@` as it does a move made elsewhere. */
export async function reviewAfter(
  workspace: WorkspaceName,
  load: (revset: string) => Promise<string | undefined>,
  revset: string,
  follow?: boolean,
) {
  if (await load(revset)) await load("@");
  if (follow) await followWorkingCopy(workspace);
}

interface Row {
  change: string;
  commit: string;
  /** Its description, when jj log gave one. */
  description?: string;
  busy: boolean;
  review(revset: string): void;
  act(action: RevAction): void;
  undo(): void;
  redo(): void;
}

/** A commit's right-click menu in the revisions panel. */
export function revRowMenu(r: Row): CtxEntry[] {
  const { change, busy } = r;
  return [
    { label: "Review This Change", keys: "r", onSelect: () => r.review(change) },
    { label: "Review from Here to @", onSelect: () => r.review(`${change}::@`) },
    "-",
    { label: "Edit", keys: "↵", disabled: busy, onSelect: () => r.act(editAction(change)) },
    "-",
    { label: "Copy", items: copyItems(change, r.commit, r.description) },
    "-",
    { label: "Undo", keys: "u", disabled: busy, onSelect: r.undo },
    { label: "Redo", keys: "U", disabled: busy, onSelect: r.redo },
  ];
}

/** Copy items for a commit. */
export function copyItems(change: string, commit: string, description?: string): CtxEntry[] {
  return [
    { label: "Change ID", onSelect: () => copy(change) },
    { label: "Commit ID", onSelect: () => copy(commit) },
    description !== undefined && { label: "Description", onSelect: () => copy(description) },
  ];
}
