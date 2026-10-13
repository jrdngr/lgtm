import { useEffect, useRef, useState } from "react";
import { IconButton } from "./icons";

export interface MenuItem {
  label: string;
  onSelect(): void;
  disabled?: boolean;
  title?: string;
  /** Destructive: shown in red, last. */
  danger?: boolean;
}

interface Props {
  items: (MenuItem | false | undefined)[];
  label?: string;
  className?: string;
}

/** A ⋯ button opening a short list of actions under it. Closes on a pick, Escape, or a click
 * outside. */
export function Menu({ items, label = "More actions", className }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const shown = items.filter((i): i is MenuItem => !!i);

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
    };
  }, [open]);

  if (shown.length === 0) return null;
  return (
    <div className={`menu${className ? ` ${className}` : ""}`} ref={ref}>
      <IconButton
        icon="more"
        label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        className={open ? "open" : undefined}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      />
      {open && (
        <div className="menu-list" role="menu">
          {shown.map((item) => (
            <button
              type="button"
              role="menuitem"
              key={item.label}
              className={item.danger ? "danger" : undefined}
              disabled={item.disabled}
              title={item.title}
              onClick={(e) => {
                e.stopPropagation();
                setOpen(false);
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
