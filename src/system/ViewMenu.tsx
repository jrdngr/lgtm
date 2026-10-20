import { useEffect, useRef } from "react";
import type { ArchViewDto } from "../bindings";
import { Chevron } from "./glyphs";

type Flag = "untracked" | "tests" | "generated" | "small" | "outside" | "groups";

const SHOWN: [Flag, string][] = [
  ["untracked", "Untracked files"],
  ["tests", "Tests"],
  ["generated", "Generated code"],
  ["small", "Small containers"],
  ["outside", "People and systems"],
  ["groups", "Groups"],
];

const LABELS: [ArchViewDto["labels"], string][] = [
  ["on", "On"],
  ["hover", "Hover"],
  ["off", "Off"],
];

const UNAFFECTED: [ArchViewDto["unaffected"], string][] = [
  ["hide", "Hide"],
  ["fade", "Fade"],
];

/** Opens View. */
export function ViewButton({ open, onToggle }: { open: boolean; onToggle(): void }) {
  return (
    <button
      type="button"
      className={`sys-tool${open ? " on" : ""}`}
      aria-expanded={open}
      data-view
      onClick={onToggle}
    >
      View
      <span className="sys-down">
        <Chevron size={10} />
      </span>
    </button>
  );
}

interface PopProps {
  view: ArchViewDto;
  /** Whether a review is open, which adds the lens's choice. */
  review: boolean;
  onChange(change: Partial<ArchViewDto>): void;
  onClose(): void;
}

/** What the map shows. */
export function ViewPop({ view, review, onChange, onClose }: PopProps) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: PointerEvent) => {
      const at = e.target as Element;
      if (!root.current?.contains(at) && !at.closest?.("[data-view]")) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: keeps clicks off what's under it
    <div
      className="sys-genpop sys-viewpop"
      role="menu"
      aria-label="View"
      ref={root}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {SHOWN.map(([key, label]) => (
        <label key={key} className="sys-genrow">
          <input
            type="checkbox"
            checked={view[key]}
            onChange={() => onChange({ [key]: !view[key] })}
          />
          <span className="sys-genname">{label}</span>
        </label>
      ))}
      <div className="sys-viewsep" />
      <Choice
        label="Edge labels"
        options={LABELS}
        value={view.labels}
        onPick={(labels) => onChange({ labels })}
      />
      {review && (
        <Choice
          label="Unaffected"
          options={UNAFFECTED}
          value={view.unaffected}
          onPick={(unaffected) => onChange({ unaffected })}
        />
      )}
    </div>
  );
}

function Choice<T extends string>(p: {
  label: string;
  options: [T, string][];
  value: T;
  onPick(v: T): void;
}) {
  return (
    <div className="sys-viewrow">
      <span className="sys-genname">{p.label}</span>
      <span className="sys-levels">
        {p.options.map(([v, name]) => (
          <button
            key={v}
            type="button"
            className={v === p.value ? "on" : undefined}
            aria-pressed={v === p.value}
            onClick={() => p.onPick(v)}
          >
            {name}
          </button>
        ))}
      </span>
    </div>
  );
}
