import { type ReactNode, useState } from "react";

/** Fixed and dismissed findings, folded to one line at the end of a list. */
export function SettledFold({ count, children }: { count: number; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="settled-fold">
      <button type="button" className="quiet" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? "▾" : "▸"} Fixed or dismissed · {count}
      </button>
      {open && children}
    </div>
  );
}
