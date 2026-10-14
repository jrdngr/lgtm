import type { ReactNode } from "react";

/** A spinning ring. */
export function Spinner() {
  return <span className="spinner" aria-hidden />;
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
