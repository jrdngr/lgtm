import type { JjAction } from "../bindings";

/** What a motion acts on: the commit under the cursor and the picked range, as revsets. */
export interface MotionTarget {
  cursor: string;
  picked: string;
  /** The bookmark picked or named for a motion that asks for one. */
  bookmark?: string;
  /** The description typed for a motion that asks for one. */
  description?: string;
}

/** A jj action, or a checkpoint of the working copy. */
export type MotionAction = JjAction | { kind: "checkpoint" };

export interface Motion {
  keys: string;
  label: string;
  action(t: MotionTarget): MotionAction;
  /** The review follows `@` after it, as after an edit. */
  follow?: boolean;
  /** It moves one of the bookmarks nearest the cursor, picked first when there are several. */
  pickBookmark?: boolean;
  /** It takes a new bookmark's name or the cursor commit's description, typed first. */
  ask?: "name" | "description";
  /** Off `@`, it acts on `@` or the cursor's commit, picked first. */
  pickRev?: boolean;
}

/** The ⌘J motions, in groups. `trunk` names trunk's bookmark for the rebase's label. */
export function motions(trunk: string | null): Motion[][] {
  return [
    [
      {
        keys: "na",
        label: "New after",
        action: (t) => ({ kind: "newAfter", rev: t.cursor }),
        follow: true,
      },
      {
        keys: "nb",
        label: "New before",
        action: (t) => ({ kind: "newBefore", rev: t.cursor }),
        follow: true,
      },
    ],
    [
      {
        keys: "d",
        label: "Describe",
        action: (t) => ({ kind: "describe", rev: t.cursor, message: t.description ?? "" }),
        ask: "description",
      },
    ],
    [
      {
        keys: "s",
        label: "Squash into parent",
        action: (t) => ({ kind: "squash", revs: t.picked }),
      },
    ],
    [
      { keys: "ba", label: "Advance bookmark", action: () => ({ kind: "bookmarkAdvance" }) },
      {
        keys: "bc",
        label: "Create bookmark",
        action: (t) => ({ kind: "bookmarkCreate", name: t.bookmark ?? "", rev: t.cursor }),
        ask: "name",
      },
      {
        keys: "bs",
        label: "Set bookmark",
        action: (t) => ({ kind: "bookmarkSet", name: t.bookmark ?? "", rev: t.cursor }),
        pickBookmark: true,
      },
    ],
    [
      { keys: "u", label: "Undo", action: () => ({ kind: "undo", steps: 1 }) },
      { keys: "U", label: "Redo", action: () => ({ kind: "redo", steps: 1 }) },
    ],
    [{ keys: "c", label: "Checkpoint", action: () => ({ kind: "checkpoint" }) }],
    [
      { keys: "gf", label: "Fetch", action: () => ({ kind: "fetch" }) },
      { keys: "gp", label: "Push", action: () => ({ kind: "push" }) },
      {
        keys: "gr",
        label: `Rebase onto ${trunk ? `${trunk}@origin` : "trunk"}`,
        action: (t) => ({ kind: "rebaseOnTrunk", rev: t.cursor }),
        pickRev: true,
      },
    ],
  ];
}

/** The groups' motions that start with `typed`, empty groups dropped. */
export function narrow(groups: Motion[][], typed: string): Motion[][] {
  return groups.map((g) => g.filter((m) => m.keys.startsWith(typed))).filter((g) => g.length);
}

/** After `key`, typed so far `typed`: the motion to run, the longer prefix to wait on, or
 * nothing when no motion starts that way. */
export function step(
  groups: Motion[][],
  typed: string,
  key: string,
): { run: Motion } | { typed: string } | undefined {
  const next = typed + key;
  const all = groups.flat();
  const run = all.find((m) => m.keys === next);
  if (run) return { run };
  return all.some((m) => m.keys.startsWith(next)) ? { typed: next } : undefined;
}

/** A description's first line, which Describe edits. */
export const title = (description: string) => description.split("\n", 1)[0];

/** `description` with its first line made `line`, keeping the body. */
export function retitle(description: string, line: string): string {
  const body = description.indexOf("\n");
  return body < 0 ? line : line + description.slice(body);
}
