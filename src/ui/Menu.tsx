import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { IconButton } from "./icons";

export interface MenuItem {
  label: string;
  onSelect(): void;
  disabled?: boolean;
  title?: string;
  /** Destructive: shown in red. */
  danger?: boolean;
}

interface Props {
  items: (MenuItem | false | undefined)[];
  label?: string;
  /** Named button in place of the ⋯ icon. */
  text?: string;
  className?: string;
}

/** Where the list sits: fixed to the window and portaled to the body, so a scrolling side panel
 * or a transformed ancestor can't clip or shift it. */
interface Place {
  right: number;
  top?: number;
  bottom?: number;
}

/** Room the list needs below the button before it opens upward instead. */
const ROOM = 160;

function placeBy(button: Element): Place {
  const r = button.getBoundingClientRect();
  const right = window.innerWidth - r.right;
  return window.innerHeight - r.bottom < ROOM && r.top > ROOM
    ? { right, bottom: window.innerHeight - r.top + 2 }
    : { right, top: r.bottom + 2 };
}

/** A ⋯ button opening a short list of actions under it, or over it near the window's bottom.
 * Closes on a pick, Escape, a click outside, or a scroll. */
export function Menu({ items, label = "More actions", text, className }: Props) {
  const [place, setPlace] = useState<Place>();
  const open = place !== undefined;
  const close = () => setPlace(undefined);
  const ref = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const shown = items.filter((i): i is MenuItem => !!i);

  useEffect(() => {
    if (!open) return;
    const shut = () => setPlace(undefined);
    const inside = (e: Event) => [ref, listRef].some((r) => r.current?.contains(e.target as Node));
    const away = (e: PointerEvent) => {
      if (!inside(e)) shut();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      shut();
    };
    // The list stays where it opened, so it closes rather than drift from its button.
    const scrolled = (e: Event) => {
      if (!inside(e)) shut();
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    window.addEventListener("scroll", scrolled, true);
    window.addEventListener("resize", shut);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
      window.removeEventListener("scroll", scrolled, true);
      window.removeEventListener("resize", shut);
    };
  }, [open]);

  const toggle = (e: React.MouseEvent<HTMLElement>) => {
    e.stopPropagation();
    setPlace(open ? undefined : placeBy(e.currentTarget));
  };
  if (shown.length === 0) return null;
  return (
    <div className={`menu${className ? ` ${className}` : ""}`} ref={ref}>
      {text ? (
        <button
          type="button"
          className={`sys-btn${open ? " open" : ""}`}
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={toggle}
        >
          {text} ▾
        </button>
      ) : (
        <IconButton
          icon="more"
          label={label}
          aria-haspopup="menu"
          aria-expanded={open}
          className={open ? "open" : undefined}
          onClick={toggle}
        />
      )}
      {open &&
        createPortal(
          <div className="menu-list" role="menu" style={place} ref={listRef}>
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
                  close();
                  item.onSelect();
                }}
              >
                {item.label}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
