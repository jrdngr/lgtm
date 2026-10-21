import type { JjAction } from "../bindings";
import { Busy } from "../ui/Busy";

/** Key groups, each as key caps and what they do. */
const HINTS: [string[], string][] = [
  [["r"], "review"],
  [["↵"], "edit"],
  [["⌘J"], "motions"],
];

const RUNNING: Partial<Record<JjAction["kind"], string>> = {
  fetch: "Fetching…",
  push: "Pushing…",
  rebaseOnTrunk: "Rebasing…",
  squash: "Squashing…",
};

interface Props {
  /** The jj action running, if one is. */
  running?: JjAction;
  /** What the last fetch or push did. */
  note?: string;
  /** A review is loading. */
  loading: boolean;
}

/** The revisions log's keys, and what it's doing or last did. */
export function RevisionsFooter({ running, note, loading }: Props) {
  return (
    <div className="picker-footer">
      {HINTS.map(([keys, label]) => (
        <span key={label}>
          {keys.map((k) => (
            <kbd key={k}>{k}</kbd>
          ))}{" "}
          {label}
        </span>
      ))}
      <span className="spacer" />
      {running ? (
        <Busy>{RUNNING[running.kind] ?? "Working…"}</Busy>
      ) : loading ? (
        <Busy>Loading review…</Busy>
      ) : (
        note && <span className="picker-note">{note}</span>
      )}
    </div>
  );
}
