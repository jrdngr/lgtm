import { Busy } from "../ui/Busy";

/** Key groups, each as key caps and what they do. */
const HINTS: [string[], string][] = [
  [["r"], "review"],
  [["↵"], "edit"],
  [["⌘J"], "motions"],
];

interface Props {
  /** What the last fetch or push did. */
  note?: string;
  /** A review is loading. */
  loading: boolean;
}

/** The revisions log's keys, a review loading, and what the last fetch or push did. */
export function RevisionsFooter({ note, loading }: Props) {
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
      {loading ? <Busy>Loading review…</Busy> : note && <span className="picker-note">{note}</span>}
    </div>
  );
}
