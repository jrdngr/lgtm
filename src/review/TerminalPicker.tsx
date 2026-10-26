import { useEffect, useRef, useState } from "react";
import type { TerminalChoice } from "../terminal/registry";
import "./MarkMenu.css";
import "./TerminalPicker.css";

interface Props {
  terminals: TerminalChoice[];
  title: string;
  onPick(id: string): void;
  onClose(): void;
}

/** Picks which of the workspace's terminals to send to. The last focused one comes first.
 * Number keys pick directly; j/k or arrows and Enter pick the highlighted one. */
export function TerminalPicker({ terminals, title, onPick, onClose }: Props) {
  const [at, setAt] = useState(0);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => ref.current?.focus(), []);

  function onKeyDown(e: React.KeyboardEvent) {
    e.stopPropagation();
    e.preventDefault();
    const n = Number(e.key);
    if (n >= 1 && n <= terminals.length) onPick(terminals[n - 1].id);
    else if (e.key === "Enter") onPick(terminals[at].id);
    else if (e.key === "Escape") onClose();
    else if (e.key === "j" || e.key === "ArrowDown") setAt((at + 1) % terminals.length);
    else if (e.key === "k" || e.key === "ArrowUp")
      setAt((at - 1 + terminals.length) % terminals.length);
  }

  return (
    <>
      <div className="menu-backdrop" onMouseDown={onClose} />
      <div ref={ref} className="mark-menu terminal-picker" tabIndex={-1} onKeyDown={onKeyDown}>
        <div className="menu-count">{title}</div>
        {terminals.map((t, i) => (
          <button
            type="button"
            key={t.id}
            className={i === at ? "active" : undefined}
            onMouseEnter={() => setAt(i)}
            onClick={() => onPick(t.id)}
            title={
              t.canPaste
                ? undefined
                : "Its program would run each line. Start your agent there first."
            }
          >
            <span className="menu-key">{i + 1}</span>
            {t.title || `Terminal ${i + 1}`}
            {!t.canPaste && <span className="picker-note">shell</span>}
          </button>
        ))}
      </div>
    </>
  );
}
