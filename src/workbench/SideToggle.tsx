interface Props {
  side: "left" | "right";
  open: boolean;
  shortcut: string;
  onClick(): void;
}

/** The standard "toggle sidebar" icon: a window with its left or right column filled while
 * that panel is shown. */
export function SideToggle({ side, open, shortcut, onClick }: Props) {
  const x = side === "left" ? 2 : 10;
  const label = `${open ? "Hide" : "Show"} ${side} panel (${shortcut})`;
  return (
    <button
      type="button"
      className="icon-button"
      onClick={onClick}
      title={label}
      aria-label={label}
      aria-pressed={open}
    >
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
        <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" fill="none" stroke="currentColor" />
        <line
          x1={side === "left" ? 6 : 10}
          x2={side === "left" ? 6 : 10}
          y1="3"
          y2="13"
          stroke="currentColor"
        />
        {open && <rect x={x} y="3" width="4" height="10" fill="currentColor" />}
      </svg>
    </button>
  );
}
