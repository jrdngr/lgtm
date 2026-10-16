import type { ChangeDto } from "../bindings";

interface Props {
  changes: ChangeDto[];
  /** Pastes text into one of the workspace's terminals. */
  send(text: string): void;
}

const terminalPrompt = (change: string) =>
  `Resolve the conflicts in jj commit ${change}: run \`jj new ${change}\`, fix the conflict markers ` +
  "it shows (`jj resolve --list`), then run `jj squash` to move the resolution into it.";

/** Conflicted changes in the review, each of which can go to a terminal agent to resolve. */
export function Conflicts({ changes, send }: Props) {
  const conflicted = changes.filter((c) => c.conflict);
  if (conflicted.length === 0) return null;
  return (
    <>
      {conflicted.map((c) => (
        <div key={c.changeId} className="review-note warn">
          {c.changeId} {c.description || "(no description)"} has conflicts.
          <button type="button" onClick={() => send(terminalPrompt(c.changeId))}>
            Send to terminal
          </button>
        </div>
      ))}
    </>
  );
}
