import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { MarkSlot } from "../bindings";
import { saveSettings } from "../settings/store";
import { IconButton } from "../ui/icons";
import { type MarkSetup, markVar } from "./marks";
import "./MarkPalette.css";

/** `slots` with the slot at `from` moved to `to`. */
export function moved(slots: MarkSlot[], from: number, to: number): MarkSlot[] {
  const next = [...slots];
  const [slot] = next.splice(from, 1);
  next.splice(to, 0, slot);
  return next;
}

/** `slots` with `color` shown or hidden. A shown color goes after the others in use, so it
 * takes the next number key. */
export function withShown(slots: MarkSlot[], color: MarkSlot["color"], shown: boolean) {
  const rest = slots.filter((s) => s.color !== color);
  const slot = { ...(slots.find((s) => s.color === color) as MarkSlot), shown };
  const at = rest.filter((s) => s.shown).length;
  return [...rest.slice(0, at), slot, ...rest.slice(at)];
}

interface Props {
  setup: MarkSetup;
  /** Where the palette sits, fixed to the window. */
  place: { top: number; right: number };
  onClose(): void;
}

/** Every mark color: the ones in use, to rename, drag into key order, count as done or
 * remove; then the rest, to add. Changes save to the settings at once. */
export function MarkPalette({ setup, place, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string>();
  const [dragging, setDragging] = useState<number>();
  const shown = setup.shown;
  const hidden = setup.slots.filter((s) => !s.shown);

  useEffect(() => {
    // The toggle button closes the palette itself.
    const away = (e: PointerEvent) => {
      const t = e.target as Element;
      if (!ref.current?.contains(t) && !t.closest?.(".mark-palette-toggle")) onClose();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  async function save(marks: MarkSlot[]) {
    const failed = await saveSettings({ marks });
    setError(failed);
  }

  const update = (color: MarkSlot["color"], patch: Partial<MarkSlot>) =>
    save(setup.slots.map((s) => (s.color === color ? { ...s, ...patch } : s)));

  return createPortal(
    <div className="mark-palette" ref={ref} style={place}>
      <div className="mark-palette-head">Mark colors</div>
      {shown.map((s, i) => (
        <div
          key={s.color}
          className={`mark-palette-row${dragging === i ? " dragging" : ""}`}
          onDragOver={(e) => {
            if (dragging === undefined) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
          }}
          onDrop={(e) => {
            e.preventDefault();
            if (dragging !== undefined && dragging !== i) {
              // Rows index the shown colors, which hidden ones can sit among in `slots`.
              const at = (row: number) => setup.slots.indexOf(shown[row]);
              save(moved(setup.slots, at(dragging), at(i)));
            }
            setDragging(undefined);
          }}
        >
          <span
            className="grip"
            title="Drag to change its number key"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.effectAllowed = "move";
              const row = e.currentTarget.parentElement;
              if (row) e.dataTransfer.setDragImage(row, 0, 0);
              setDragging(i);
            }}
            onDragEnd={() => setDragging(undefined)}
          >
            ⋮⋮
          </span>
          <span className="swatch" style={{ background: markVar(s.color) }} />
          <LabelInput slot={s} onSave={(label) => update(s.color, { label })} />
          <label className="done" title="Rows with this mark count as done">
            <input
              type="checkbox"
              checked={s.done}
              onChange={(e) => update(s.color, { done: e.target.checked })}
            />
            done
          </label>
          <kbd>{i + 1}</kbd>
          <IconButton
            icon="close"
            label={`Remove ${setup.label(s.color)}. Its marks stay`}
            onClick={() => save(withShown(setup.slots, s.color, false))}
          />
        </div>
      ))}
      {hidden.length > 0 && <div className="mark-palette-divider" />}
      {hidden.map((s) => (
        <div key={s.color} className="mark-palette-row off">
          <span className="grip" />
          <span className="swatch" style={{ background: markVar(s.color) }} />
          <span className="name">{setup.label(s.color)}</span>
          <IconButton
            icon="plus"
            label={shown.length < 9 ? `Add on key ${shown.length + 1}` : "Add"}
            onClick={() => save(withShown(setup.slots, s.color, true))}
          />
        </div>
      ))}
      <div className="mark-palette-foot">
        {error ?? "Drag to reorder: the order sets the number keys. 0 clears a mark."}
      </div>
    </div>,
    document.body,
  );
}

/** A label that saves when it loses focus or on Enter. */
function LabelInput({ slot, onSave }: { slot: MarkSlot; onSave(label: string): void }) {
  const [value, setValue] = useState(slot.label);
  useEffect(() => setValue(slot.label), [slot.label]);
  const commit = () => value.trim() !== slot.label && onSave(value.trim());
  return (
    <input
      className="label"
      value={value}
      placeholder="add label"
      aria-label={`Label for ${slot.color}`}
      onChange={(e) => setValue(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
      }}
    />
  );
}
