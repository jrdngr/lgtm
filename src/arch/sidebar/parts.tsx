import { type ReactNode, useState } from "react";
import { GroupHead } from "../../ui/GroupHead";
import { meterTone } from "./logic";

/** A folding list: its name and count, then its rows. */
export function Fold({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children: ReactNode;
}) {
  const [shut, setShut] = useState(false);
  return (
    <>
      <GroupHead name={title} count={count} shut={shut} onToggle={() => setShut(!shut)} />
      {!shut && children}
    </>
  );
}

export function Box({ on, label, onToggle }: { on: boolean; label: string; onToggle(): void }) {
  return (
    <input
      type="checkbox"
      className="arch-box"
      aria-label={label}
      checked={on}
      onChange={onToggle}
    />
  );
}

/** A row that selects something in the tab. */
export function Row({
  cur,
  className,
  onClick,
  children,
}: {
  cur: boolean;
  className?: string;
  onClick(e: React.MouseEvent): void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`arch-row${className ? ` ${className}` : ""}`}
      aria-current={cur || undefined}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** Five segments, as many filled as `score`, and `n/5` once there's a score. */
export function Meter({ score }: { score: number }) {
  return (
    <span className={`arch-meter ${meterTone(score)}`}>
      <span className="arch-meter-bar">
        {[1, 2, 3, 4, 5].map((k) => (
          <i key={k} className={k <= score ? "on" : undefined} />
        ))}
      </span>
      {score > 0 && (
        <b>
          {score}
          <small>/5</small>
        </b>
      )}
    </span>
  );
}
