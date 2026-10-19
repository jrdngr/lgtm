import type { JjAction } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { confirmAction, promptText } from "../ui/prompt";
import { followWorkingCopy } from "./followWorkingCopy";

/** An action on a commit: the jj action, its label, and whether the review follows `@` after
 * it. */
export type RevAction = [JjAction, string, boolean?];

/** Keys that act on the highlighted commit. */
export const REV_ACTIONS: Record<string, (rev: string) => RevAction> = {
  e: (rev) => [{ kind: "edit", rev }, `Editing ${rev}`, true],
  n: (rev) => [{ kind: "newAfter", rev }, `New commit after ${rev}`],
  N: (rev) => [{ kind: "newBefore", rev }, `New commit before ${rev}`],
  s: (rev) => [{ kind: "squash", rev }, `Squashed ${rev}`],
};

/** The review grown to take in `change`, with the commits between. */
export const addToReview = (revset: string, change: string) => `connected((${revset}) | ${change})`;

/** Revsets offered for review: `@`, its parent, and everything since trunk, named by trunk's
 * bookmark when it has one. */
export function presets(trunk: string | null) {
  return ["@", "@-", `${trunk ?? "trunk()"}..@`];
}

/** Asks for `rev`'s new description, starting from its current one. */
export async function askDescribe(rev: string, current: string): Promise<RevAction | undefined> {
  const message = await promptText(`Describe ${rev}`, {
    value: current,
    action: "Describe",
    multiline: true,
  });
  return message === undefined
    ? undefined
    : [{ kind: "describe", rev, message }, `Described ${rev}`];
}

/** Asks before abandoning `rev`. */
export async function askAbandon(rev: string): Promise<RevAction | undefined> {
  const ok = await confirmAction(
    `Abandon ${rev}? Its children move onto its parent. Undo brings it back.`,
    "Abandon",
  );
  return ok ? [{ kind: "abandon", rev }, `Abandoned ${rev}`] : undefined;
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
  /** The review's revset. */
  revset: string;
  busy: boolean;
  review(revset: string): void;
  act(action: RevAction): void;
  undo(): void;
  redo(): void;
}

/** A commit's right-click menu in the revisions panel. */
export function revRowMenu(r: Row): CtxEntry[] {
  const { change, busy } = r;
  const key = (k: string, label: string) => ({
    label,
    keys: k,
    disabled: busy,
    onSelect: () => r.act(REV_ACTIONS[k](change)),
  });
  const ask = (f: () => Promise<RevAction | undefined>) => () => f().then((a) => a && r.act(a));
  return [
    { label: "Review This Change", keys: "r", onSelect: () => r.review(change) },
    { label: "Add to Review", keys: "⇧r", onSelect: () => r.review(addToReview(r.revset, change)) },
    { label: "Review from Here to @", onSelect: () => r.review(`${change}::@`) },
    "-",
    { ...key("e", "Edit"), keys: "↵" },
    key("n", "New After"),
    key("N", "New Before"),
    key("s", "Squash into Parent"),
    {
      label: "Describe…",
      disabled: busy,
      onSelect: ask(() => askDescribe(change, r.description ?? "")),
    },
    { label: "Abandon…", disabled: busy, onSelect: ask(() => askAbandon(change)) },
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
