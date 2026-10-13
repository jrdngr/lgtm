import { useState } from "react";
import type { MarkColor } from "../bindings";
import { IconButton } from "../ui/icons";
import { MarkPalette } from "./MarkPalette";
import { type MarkSetup, markInk, markVar } from "./marks";
import "./MarkBar.css";

interface Props {
  setup: MarkSetup;
  /** Marks the selection; disabled with no review. */
  onMark?(color: MarkColor): void;
  onFilter(color: MarkColor): void;
}

/** The mark colors in use, in key order: labeled ones as a box with the label, the rest as a
 * square. Clicking one marks the selection and right-clicking filters to it. The + opens the
 * palette. */
export function MarkBar({ setup, onMark, onFilter }: Props) {
  const [place, setPlace] = useState<{ top: number; right: number }>();
  return (
    <span className="mark-bar">
      {setup.shown.map((s, i) => {
        const label = s.label.trim();
        return (
          <button
            type="button"
            key={s.color}
            className={`mark-chip${label ? "" : " square"}`}
            style={{ background: markVar(s.color), color: markInk(s.color) }}
            title={`${label || s.color}: press ${i + 1} to mark the selection. Right-click to show only these rows`}
            disabled={!onMark}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onMark?.(s.color)}
            onContextMenu={(e) => {
              e.preventDefault();
              onFilter(s.color);
            }}
          >
            <kbd>{i + 1}</kbd>
            {label}
          </button>
        );
      })}
      <IconButton
        icon="plus"
        label="Mark colors"
        className={`mark-palette-toggle${place ? " open" : ""}`}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setPlace(place ? undefined : { top: r.bottom + 4, right: window.innerWidth - r.right });
        }}
      />
      {place && <MarkPalette setup={setup} place={place} onClose={() => setPlace(undefined)} />}
    </span>
  );
}
