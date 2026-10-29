import type { ReactNode } from "react";

/** A spinning ring. */
export function Spinner() {
  return (
    <svg className="spinner" viewBox="0 0 16 16" aria-hidden>
      <circle className="spinner-track" cx="8" cy="8" r="6.5" />
      <circle className="spinner-arc" cx="8" cy="8" r="6.5" pathLength="100" />
    </svg>
  );
}

/** Text saying something is happening, with a spinner before it. */
export function Busy({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={`busy${className ? ` ${className}` : ""}`}>
      <Spinner />
      {children}
    </span>
  );
}
