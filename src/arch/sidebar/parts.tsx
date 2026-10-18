import type { ReactNode } from "react";
import { dockOf } from "../../core/docks";
import { openArchitecture } from "../../core/panels";
import type { WorkspaceName } from "../../core/workspace";
import { type ArchSelection, setArchView } from "../state";
import { meterTone } from "./logic";

/** A list's title and count, with its actions. */
export function Sec({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children?: ReactNode;
}) {
  return (
    <div className="arch-sec">
      <span className="arch-sec-title">{title}</span>
      <span className="arch-sec-count">{count}</span>
      <span className="spacer" />
      {children}
    </div>
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

/** Five segments, as many filled as `score`, with `n/5` once there's a score. */
export function Meter({ score }: { score: number }) {
  return (
    <span className={`arch-meter ${meterTone(score)}`}>
      {[1, 2, 3, 4, 5].map((k) => (
        <i key={k} className={k <= score ? "on" : undefined} />
      ))}
      {score > 0 && (
        <b>
          {score}
          <small>/5</small>
        </b>
      )}
    </span>
  );
}

/** Selects `selection` and shows it in the Architecture tab. */
export function showInTab(workspace: WorkspaceName, review: number, selection: ArchSelection) {
  setArchView(review, { selection });
  const api = dockOf(workspace);
  if (api) openArchitecture(api);
}
