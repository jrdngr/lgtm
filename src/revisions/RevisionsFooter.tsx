import { Busy } from "../ui/Busy";

/** Key groups, each as key caps and what they do. */
const HINTS: [string[], string][] = [
  [["r"], "review"],
  [["e"], "edit"],
];

interface Props {
  busy: boolean;
  /** A review is loading. */
  loading: boolean;
}

/** The revisions log's keys, and what it's doing. */
export function RevisionsFooter({ busy, loading }: Props) {
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
      {busy ? <Busy>Working…</Busy> : loading && <Busy>Loading review…</Busy>}
    </div>
  );
}
