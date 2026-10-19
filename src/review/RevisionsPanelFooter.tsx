import { Busy } from "../ui/Busy";

/** Key groups, each as key caps and what they do. */
const HINTS: [string[], string][] = [
  [["r"], "review"],
  [["e"], "edit"],
];

interface Props {
  last?: string;
  busy: boolean;
  /** A review is loading. */
  loading: boolean;
}

/** The revisions panel's keys, and the last action with its undo key. */
export function RevisionsPanelFooter({ last, busy, loading }: Props) {
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
      {busy ? (
        <Busy>Working…</Busy>
      ) : loading ? (
        <Busy>Loading review…</Busy>
      ) : (
        last && (
          <span className="picker-last">
            {last} · <kbd>u</kbd>
          </span>
        )
      )}
    </div>
  );
}
