import { useEffect, useRef } from "react";
import { IconButton } from "../ui/icons";
import "./find.css";

export interface FindState {
  query: string;
  matchCase: boolean;
}

interface Props extends FindState {
  /** The current match, from 0, and how many there are. */
  at: number;
  total: number;
  /** Changes when the bar should take focus again, as ⌘F does while it's open. */
  focusKey?: number;
  onChange(next: FindState): void;
  onStep(back: boolean): void;
  onClose(): void;
}

/** A find box over the top right of what it searches. */
export function FindBar({
  query,
  matchCase,
  at,
  total,
  focusKey,
  onChange,
  onStep,
  onClose,
}: Props) {
  const input = useRef<HTMLInputElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: refocuses on each ⌘F
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, [focusKey]);
  return (
    <div
      className="find-bar"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        else if (e.key === "Enter") onStep(e.shiftKey);
        else {
          // Typing stays in the field; only chords reach the pane's own keys.
          if (!e.metaKey && !e.ctrlKey) e.stopPropagation();
          return;
        }
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      <input
        ref={input}
        value={query}
        placeholder="Find"
        spellCheck={false}
        aria-label="Find"
        onChange={(e) => onChange({ query: e.target.value, matchCase })}
      />
      <button
        type="button"
        className={`find-case${matchCase ? " on" : ""}`}
        aria-label="Match Case"
        aria-pressed={matchCase}
        onClick={() => onChange({ query, matchCase: !matchCase })}
      >
        Aa
      </button>
      <span className={`find-count${query && !total ? " none" : ""}`}>
        {query ? (total ? `${at + 1} of ${total}` : "No results") : ""}
      </span>
      <IconButton
        icon="chevronUp"
        label="Previous"
        disabled={!total}
        onClick={() => onStep(true)}
      />
      <IconButton icon="chevronDown" label="Next" disabled={!total} onClick={() => onStep(false)} />
      <IconButton icon="close" label="Close" onClick={onClose} />
    </div>
  );
}
