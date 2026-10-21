import type { JjAction } from "../bindings";

/** What a motion acts on: the commit under the cursor and the picked range, as revsets. */
export interface MotionTarget {
  cursor: string;
  picked: string;
}

export interface Motion {
  keys: string;
  label: string;
  action(t: MotionTarget): JjAction;
  /** The review follows `@` after it, as after an edit. */
  follow?: boolean;
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
        keys: "s",
        label: "Squash into parent",
        action: (t) => ({ kind: "squash", revs: t.picked }),
      },
    ],
    [
      { keys: "u", label: "Undo", action: () => ({ kind: "undo", steps: 1 }) },
      { keys: "U", label: "Redo", action: () => ({ kind: "redo", steps: 1 }) },
    ],
    [
      { keys: "gf", label: "Fetch", action: () => ({ kind: "fetch" }) },
      { keys: "gp", label: "Push", action: () => ({ kind: "push" }) },
    ],
    [
      {
        keys: "rm",
        label: `Rebase onto ${trunk ? `${trunk}@origin` : "trunk"}`,
        action: (t) => ({ kind: "rebaseOnTrunk", rev: t.cursor }),
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
