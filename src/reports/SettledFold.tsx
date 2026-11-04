import { type ReactNode, useState } from "react";

/** Fixed and dismissed findings, folded to one line at the end of a list. A `reveal` it hasn't
 * seen before opens it, for a jump to one of its cards; the one it mounts with doesn't. */
export function SettledFold({
  count,
  reveal,
  children,
}: {
  count: number;
  reveal?: number;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(reveal);
  if (reveal !== undefined && reveal !== seen) {
    setSeen(reveal);
    setOpen(true);
  }
  return (
    <div className="settled-fold">
      <button type="button" className="quiet" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? "▾" : "▸"} Fixed or dismissed · {count}
      </button>
      {open && children}
    </div>
  );
}
