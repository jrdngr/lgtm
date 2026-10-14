import type { JjAction } from "../bindings";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { confirmAction, promptText } from "../ui/prompt";

/** An action on a commit: the jj action, its label, and the revset to review after it. */
export type RevAction = [JjAction, string, string?];

/** Keys that act on the highlighted commit. */
export const REV_ACTIONS: Record<string, (rev: string) => RevAction> = {
  e: (rev) => [{ kind: "edit", rev }, `Editing ${rev}`, "@"],
  n: (rev) => [{ kind: "newAfter", rev }, `New commit after ${rev}`, `${rev}::@`],
  N: (rev) => [{ kind: "newBefore", rev }, `New commit before ${rev}`, `@::${rev}`],
  s: (rev) => [{ kind: "squash", rev }, `Squashed ${rev}`],
};

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

/** Loads the review after a jj action: on `next`, or on `revset` while it still resolves and
 * on `@` once it doesn't. Returns the error of loading `next`. */
export async function reviewAfter(
  load: (revset: string) => Promise<string | undefined>,
  revset: string,
  next?: string,
): Promise<string | undefined> {
  const failed = await load(next ?? revset);
  if (!failed || next) return failed;
  await load("@");
  return undefined;
}

interface Row {
  change: string;
  commit: string;
  /** Its description, when jj log gave one. */
  description?: string;
  /** Whether ↵ reviews just this commit, rather than a range it's in. */
  alone: boolean;
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
    {
      label: "Review This Change",
      keys: r.alone ? "↵" : undefined,
      onSelect: () => r.review(change),
    },
    { label: "Review from Here to @", onSelect: () => r.review(`${change}::@`) },
    "-",
    key("e", "Edit"),
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
    { label: "Copy Change ID", onSelect: () => copy(change) },
    { label: "Copy Commit ID", onSelect: () => copy(r.commit) },
    r.description !== undefined && {
      label: "Copy Description",
      onSelect: () => copy(r.description ?? ""),
    },
    "-",
    { label: "Undo", keys: "u", disabled: busy, onSelect: r.undo },
    { label: "Redo", keys: "U", disabled: busy, onSelect: r.redo },
  ];
}
